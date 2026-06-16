use crate::models::{AppSettings, DuplicateAiAnalysis, Post};
use crate::services::deepseek;
use crate::services::duplicate;
use crate::AppState;
use anyhow::Result;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct DedupMatch {
    pub kept_post_id: i64,
    pub kept_title: String,
    pub analysis: DuplicateAiAnalysis,
}

pub struct DedupCheckOptions {
    pub exclude_post_id: Option<i64>,
    pub status_filter: Option<String>,
    pub should_cancel: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
}

impl Default for DedupCheckOptions {
    fn default() -> Self {
        Self {
            exclude_post_id: None,
            status_filter: None,
            should_cancel: None,
        }
    }
}

pub async fn check_duplicate(
    state: &AppState,
    settings: &AppSettings,
    title: &str,
    description: &str,
    options: DedupCheckOptions,
) -> Result<Option<DedupMatch>> {
    if options.should_cancel.as_ref().is_some_and(|f| f()) {
        return Ok(None);
    }

    if let Ok(exact_matches) = state.db.find_posts_by_normalized_title(title) {
        for post in exact_matches {
            if options.exclude_post_id == Some(post.id) {
                continue;
            }
            let kept_title = post.ai_title.as_deref().unwrap_or(&post.raw_title).to_string();
            return Ok(Some(DedupMatch {
                kept_post_id: post.id,
                kept_title,
                analysis: DuplicateAiAnalysis {
                    is_duplicate: true,
                    confidence: 100,
                    explanation: "Точное совпадение нормализованного заголовка".to_string(),
                },
            }));
        }
    }

    if options.should_cancel.as_ref().is_some_and(|f| f()) {
        return Ok(None);
    }

    let window_days = Some(settings.ai_duplicate_window_days);
    let limit = settings.ai_duplicate_check_limit as i64;
    let status_filter = options.status_filter.as_deref();
    let candidates: Vec<Post> = state
        .db
        .get_dedup_candidates(
            window_days,
            status_filter,
            limit,
            options.exclude_post_id,
        )?
        .into_iter()
        .filter(|post| !duplicate::is_link_roundup_title(&post.raw_title))
        .collect();

    if candidates.is_empty() {
        return Ok(None);
    }

    let dedup_family =
        duplicate::dedup_encoder_family(&settings.normalized_local_dedup_model_id());

    if let Some(lexical) =
        find_lexical_duplicate(title, dedup_family, &candidates, options.exclude_post_id)
    {
        return Ok(Some(lexical));
    }

    if options.should_cancel.as_ref().is_some_and(|f| f()) {
        return Ok(None);
    }

    let mut top_k = settings.ai_duplicate_llm_top_k as usize;
    top_k = top_k.max(50);
    top_k = top_k.min(settings.ai_duplicate_check_limit as usize);
    let llm_candidates = rank_for_llm(title, description, &candidates, top_k, dedup_family);

    if llm_candidates.is_empty() {
        return Ok(None);
    }

    if settings.duplicate_uses_local() {
        if settings.duplicate_uses_embeddings() {
            let dedup_id = settings.normalized_local_dedup_model_id();
            if let Err(e) = state
                .local_embed
                .ensure_running(settings, &dedup_id)
                .await
            {
                anyhow::bail!("Энкодер для дублей: {e}");
            }
        } else {
            let dedup_id = settings.normalized_local_dedup_model_id();
            if let Err(e) = state
                .local_llm
                .ensure_running_for_model(settings, &dedup_id)
                .await
            {
                anyhow::bail!("LLM для дублей: {e}");
            }
        }
    }

    let dedup_concurrency = if settings.duplicate_uses_embeddings() {
        settings.ai_dedup_concurrency.clamp(1, 10) as usize
    } else if settings.duplicate_uses_local() {
        settings.ai_dedup_concurrency.clamp(1, 2) as usize
    } else {
        settings.ai_dedup_concurrency.clamp(1, 10) as usize
    };

    let dup = deepseek::find_ai_duplicate_among_posts(
        &state.http_client(),
        settings,
        state.local_llm.clone(),
        state.local_embed.clone(),
        title,
        description,
        &llm_candidates,
        dedup_concurrency,
        options.should_cancel,
    )
    .await?;

    Ok(dup.map(|d| DedupMatch {
        kept_post_id: d.kept_post_id,
        kept_title: d.kept_title,
        analysis: d.analysis,
    }))
}

fn find_lexical_duplicate(
    new_title: &str,
    family: duplicate::DedupEncoderFamily,
    candidates: &[Post],
    exclude_post_id: Option<i64>,
) -> Option<DedupMatch> {
    let mut best: Option<((u32, u32, u32), DedupMatch)> = None;

    for post in candidates {
        if exclude_post_id == Some(post.id) {
            continue;
        }
        let kept_display = post.ai_title.as_deref().unwrap_or(&post.raw_title);
        if let Some((confidence, explanation)) = lexical_duplicate_verdict(
            family,
            new_title,
            &post.raw_title,
            post.ai_title.as_deref(),
        ) {
            let rank = duplicate::duplicate_match_rank_for_family(
                family,
                new_title,
                &post.raw_title,
                confidence,
            );
            let candidate = DedupMatch {
                kept_post_id: post.id,
                kept_title: kept_display.to_string(),
                analysis: DuplicateAiAnalysis {
                    is_duplicate: true,
                    confidence,
                    explanation,
                },
            };
            if best.as_ref().map_or(true, |(r, _)| rank > *r) {
                best = Some((rank, candidate));
            }
        }
    }

    best.map(|(_, m)| m)
}

fn lexical_duplicate_verdict(
    family: duplicate::DedupEncoderFamily,
    new_title: &str,
    kept_raw_title: &str,
    kept_ai_title: Option<&str>,
) -> Option<(u32, String)> {
    lexical_pair_verdict(family, new_title, kept_raw_title).or_else(|| {
        kept_ai_title
            .filter(|t| !t.trim().is_empty())
            .and_then(|ai_title| lexical_pair_verdict(family, new_title, ai_title))
    })
}

fn lexical_pair_verdict(
    family: duplicate::DedupEncoderFamily,
    new_title: &str,
    kept_title: &str,
) -> Option<(u32, String)> {
    if duplicate::titles_match(new_title, kept_title) {
        return Some((98, "Совпадение нормализованных заголовков".to_string()));
    }
    if duplicate::titles_strongly_similar_for_family(family, new_title, kept_title) {
        let common = duplicate::distinctive_common_word_count(new_title, kept_title);
        let pct = (duplicate::distinctive_word_jaccard(new_title, kept_title) * 100.0).round() as u32;
        let confidence = (88 + common as u32).min(98);
        return Some((
            confidence,
            format!("Схожие заголовки ({common} общих слов, {pct}% по словарю) — та же новость"),
        ));
    }
    if duplicate::titles_medium_similar_for_family(family, new_title, kept_title) {
        let common = duplicate::distinctive_common_word_count(new_title, kept_title);
        let confidence = (84 + common as u32).min(94);
        return Some((
            confidence,
            format!("Похожие заголовки ({common} общих слов) — та же новость"),
        ));
    }
    None
}

fn rank_for_llm(
    new_title: &str,
    new_desc: &str,
    candidates: &[Post],
    top_k: usize,
    family: duplicate::DedupEncoderFamily,
) -> Vec<Post> {
    if candidates.is_empty() || top_k == 0 {
        return Vec::new();
    }

    let mut scored: Vec<(i32, Post)> = candidates
        .iter()
        .map(|post| {
            let score = heuristic_score_for_post(new_title, new_desc, post, family);
            (score, post.clone())
        })
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.id.cmp(&a.1.id)));

    let mut selected = Vec::new();
    let mut seen = HashSet::new();

    for (_, post) in scored.iter().filter(|(s, _)| *s > 0) {
        if seen.insert(post.id) {
            selected.push(post.clone());
        }
    }

    for (_, post) in &scored {
        if selected.len() >= top_k {
            break;
        }
        if seen.insert(post.id) {
            selected.push(post.clone());
        }
    }

    selected
}

fn heuristic_score_for_post(
    new_title: &str,
    new_desc: &str,
    post: &Post,
    family: duplicate::DedupEncoderFamily,
) -> i32 {
    let ai_title = post.ai_title.as_deref().unwrap_or(&post.raw_title);
    let ai_desc = post
        .ai_text
        .as_deref()
        .unwrap_or(&post.raw_description);
    let mut score = heuristic_score(new_title, new_desc, ai_title, ai_desc, family);
    if post.ai_title.is_some() || post.ai_text.is_some() {
        score = score.max(heuristic_score(
            new_title,
            new_desc,
            &post.raw_title,
            &post.raw_description,
            family,
        ));
    }
    score
}

fn heuristic_score(
    new_title: &str,
    new_desc: &str,
    kept_title: &str,
    kept_desc: &str,
    family: duplicate::DedupEncoderFamily,
) -> i32 {
    let mut score = 0;
    if duplicate::titles_match(new_title, kept_title) {
        score += 100;
    } else if duplicate::titles_strongly_similar_for_family(family, new_title, kept_title) {
        score += 90;
    } else if duplicate::titles_medium_similar_for_family(family, new_title, kept_title) {
        score += 75;
    } else if duplicate::titles_similar(new_title, kept_title) {
        score += 60;
    }
    score += duplicate::distinctive_common_word_count(new_title, kept_title) as i32 * 3;
    if duplicate::descriptions_similar(new_desc, kept_desc) {
        score += 40;
    } else if duplicate::content_matches(new_title, new_desc, kept_title, kept_desc) {
        score += 20;
    }
    score
}

pub async fn sweep_new_posts_for_duplicates(
    state: &AppState,
    settings: &AppSettings,
    new_post_ids: &[i64],
    should_cancel: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
) -> Result<usize> {
    if new_post_ids.is_empty() || !settings.ai_duplicate_check {
        return Ok(0);
    }

    let mut sorted = new_post_ids.to_vec();
    sorted.sort_unstable();
    sorted.dedup();

    let mut removed = 0usize;
    for post_id in sorted {
        if should_cancel.as_ref().is_some_and(|f| f()) {
            break;
        }

        let post = match state.db.get_post(post_id) {
            Ok(post) => post,
            Err(_) => continue,
        };
        if post.status != "new" && post.status != "processing" {
            continue;
        }

        let dup = check_duplicate(
            state,
            settings,
            &post.raw_title,
            &post.raw_description,
            DedupCheckOptions {
                exclude_post_id: Some(post_id),
                status_filter: None,
                should_cancel: should_cancel.clone(),
            },
        )
        .await?;

        let Some(dup) = dup else {
            continue;
        };

        state.db.record_ai_duplicate(
            &post.source_url,
            &post.raw_title,
            &post.raw_description,
            Some(dup.kept_post_id),
            Some(&dup.kept_title),
            &dup.analysis,
        )?;
        state.db.delete_post(post_id)?;
        removed += 1;
    }

    Ok(removed)
}
