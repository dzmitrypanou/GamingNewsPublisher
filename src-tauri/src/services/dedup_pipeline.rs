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

    // Tier 1: exact normalized title match across full history
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

    // Tier 3: SQL candidates within configured window
    let window_days = Some(settings.ai_duplicate_window_days);
    let limit = settings.ai_duplicate_check_limit as i64;
    let status_filter = options.status_filter.as_deref();
    let candidates = state.db.get_dedup_candidates(
        window_days,
        status_filter,
        limit,
        options.exclude_post_id,
    )?;

    if candidates.is_empty() {
        return Ok(None);
    }

    // Tier 2 + 4 prep: heuristic hits always go to LLM; fill remaining slots up to top-K
    let mut top_k = settings.ai_duplicate_llm_top_k as usize;
    if settings.duplicate_uses_cloud() {
        top_k = top_k.max(50);
    }
    top_k = top_k.min(settings.ai_duplicate_check_limit as usize);
    let llm_candidates = rank_for_llm(title, description, &candidates, top_k);

    if llm_candidates.is_empty() {
        return Ok(None);
    }

    if settings.duplicate_uses_local() {
        if let Err(e) = state
            .local_llm
            .ensure_running(settings)
            .await
        {
            anyhow::bail!("LLM для дублей: {e}");
        }
    }

    let dedup_concurrency = if settings.duplicate_uses_local() {
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

fn rank_for_llm(new_title: &str, new_desc: &str, candidates: &[Post], top_k: usize) -> Vec<Post> {
    if candidates.is_empty() || top_k == 0 {
        return Vec::new();
    }

    let mut scored: Vec<(i32, Post)> = candidates
        .iter()
        .map(|post| {
            let score = heuristic_score_for_post(new_title, new_desc, post);
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

fn heuristic_score_for_post(new_title: &str, new_desc: &str, post: &Post) -> i32 {
    let ai_title = post.ai_title.as_deref().unwrap_or(&post.raw_title);
    let ai_desc = post
        .ai_text
        .as_deref()
        .unwrap_or(&post.raw_description);
    let mut score = heuristic_score(new_title, new_desc, ai_title, ai_desc);
    if post.ai_title.is_some() || post.ai_text.is_some() {
        score = score.max(heuristic_score(
            new_title,
            new_desc,
            &post.raw_title,
            &post.raw_description,
        ));
    }
    score
}

fn heuristic_score(
    new_title: &str,
    new_desc: &str,
    kept_title: &str,
    kept_desc: &str,
) -> i32 {
    let mut score = 0;
    if duplicate::titles_match(new_title, kept_title) {
        score += 100;
    } else if duplicate::titles_similar(new_title, kept_title) {
        score += 60;
    }
    if duplicate::descriptions_similar(new_desc, kept_desc) {
        score += 40;
    } else if duplicate::content_matches(new_title, new_desc, kept_title, kept_desc) {
        score += 20;
    }
    score
}
