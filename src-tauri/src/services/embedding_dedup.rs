use crate::local_embed_runtime::LocalEmbedRuntime;
use crate::models::{AppSettings, DuplicateAiAnalysis};
use crate::services::duplicate;
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;

#[derive(Debug, Clone, Copy)]
struct EmbedThresholds {
    full_strong: f32,
    full_with_title: f32,
    title: f32,
    full_with_lexical: f32,
    titles_similar_full: f32,
    titles_similar_title: f32,
    distinctive_common_words: usize,
    distinctive_min_with_title: usize,
    distinctive_min_titles_similar: usize,
    distinctive_high_overlap: usize,
}

fn thresholds_for_model(model_id: &str) -> EmbedThresholds {
    match duplicate::dedup_encoder_family(model_id) {
        duplicate::DedupEncoderFamily::E5Large => EmbedThresholds {
            full_strong: 0.86,
            full_with_title: 0.80,
            title: 0.82,
            full_with_lexical: 0.74,
            titles_similar_full: 0.64,
            titles_similar_title: 0.74,
            distinctive_common_words: 4,
            distinctive_min_with_title: 4,
            distinctive_min_titles_similar: 4,
            distinctive_high_overlap: 4,
        },
        duplicate::DedupEncoderFamily::Bge => EmbedThresholds {
            full_strong: 0.86,
            full_with_title: 0.80,
            title: 0.82,
            full_with_lexical: 0.74,
            titles_similar_full: 0.66,
            titles_similar_title: 0.74,
            distinctive_common_words: 3,
            distinctive_min_with_title: 3,
            distinctive_min_titles_similar: 3,
            distinctive_high_overlap: 4,
        },
        duplicate::DedupEncoderFamily::E5Base | duplicate::DedupEncoderFamily::Other => {
            EmbedThresholds {
                full_strong: 0.88,
                full_with_title: 0.82,
                title: 0.84,
                full_with_lexical: 0.76,
                titles_similar_full: 0.68,
                titles_similar_title: 0.78,
                distinctive_common_words: 2,
                distinctive_min_with_title: 2,
                distinctive_min_titles_similar: 2,
                distinctive_high_overlap: 4,
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum EmbedSide {
    Query,
    Passage,
}

pub async fn compare_news_pair_embeddings(
    client: &Client,
    settings: &AppSettings,
    embed_runtime: &LocalEmbedRuntime,
    model_id: &str,
    title_a: &str,
    description_a: &str,
    title_b: &str,
    description_b: &str,
) -> Result<DuplicateAiAnalysis> {
    embed_runtime.ensure_running(settings, model_id).await?;

    let (full_sim, title_sim) = tokio::try_join!(
        embed_pair_similarity(
            client,
            embed_runtime,
            model_id,
            title_a,
            description_a,
            title_b,
            description_b,
        ),
        embed_title_similarity(client, embed_runtime, model_id, title_a, title_b,),
    )?;

    Ok(analysis_from_scores(
        model_id,
        full_sim,
        title_sim,
        title_a,
        description_a,
        title_b,
        description_b,
    ))
}

pub async fn compare_news_to_kept_post_embeddings(
    client: &Client,
    settings: &AppSettings,
    embed_runtime: &LocalEmbedRuntime,
    model_id: &str,
    new_title: &str,
    new_description: &str,
    kept_raw_title: &str,
    kept_raw_description: &str,
    kept_ai_title: Option<&str>,
    kept_ai_text: Option<&str>,
) -> Result<DuplicateAiAnalysis> {
    embed_runtime.ensure_running(settings, model_id).await?;

    let mut kept_variants: Vec<(&str, &str)> = vec![(kept_raw_title, kept_raw_description)];
    if let Some(ai_title) = kept_ai_title.filter(|t| !t.trim().is_empty()) {
        let ai_text = kept_ai_text.unwrap_or(kept_raw_description);
        let raw_pair = combined_text(kept_raw_title, kept_raw_description);
        let ai_pair = combined_text(ai_title, ai_text);
        if ai_pair != raw_pair {
            kept_variants.push((ai_title, ai_text));
        }
    }

    let mut best_full = 0.0f32;
    let mut best_title = 0.0f32;
    let mut best_kept_title = kept_raw_title;
    let mut best_kept_description = kept_raw_description;

    for (kept_title, kept_description) in kept_variants {
        let (full_sim, title_sim) = tokio::try_join!(
            embed_pair_similarity(
                client,
                embed_runtime,
                model_id,
                new_title,
                new_description,
                kept_title,
                kept_description,
            ),
            embed_title_similarity(client, embed_runtime, model_id, new_title, kept_title,),
        )?;
        if full_sim > best_full {
            best_full = full_sim;
            best_title = title_sim;
            best_kept_title = kept_title;
            best_kept_description = kept_description;
        }
    }

    Ok(analysis_from_scores(
        model_id,
        best_full,
        best_title,
        new_title,
        new_description,
        best_kept_title,
        best_kept_description,
    ))
}

async fn embed_pair_similarity(
    client: &Client,
    embed_runtime: &LocalEmbedRuntime,
    model_id: &str,
    title_a: &str,
    description_a: &str,
    title_b: &str,
    description_b: &str,
) -> Result<f32> {
    let text_a = prepare_embed_text(model_id, EmbedSide::Query, title_a, description_a);
    let text_b = prepare_embed_text(model_id, EmbedSide::Passage, title_b, description_b);
    let (emb_a, emb_b) = tokio::try_join!(
        embed_text(client, embed_runtime, &text_a),
        embed_text(client, embed_runtime, &text_b),
    )?;
    Ok(cosine_similarity(&emb_a, &emb_b))
}

async fn embed_title_similarity(
    client: &Client,
    embed_runtime: &LocalEmbedRuntime,
    model_id: &str,
    title_a: &str,
    title_b: &str,
) -> Result<f32> {
    let text_a = prepare_embed_text(model_id, EmbedSide::Query, title_a, "");
    let text_b = prepare_embed_text(model_id, EmbedSide::Passage, title_b, "");
    let (emb_a, emb_b) = tokio::try_join!(
        embed_text(client, embed_runtime, &text_a),
        embed_text(client, embed_runtime, &text_b),
    )?;
    Ok(cosine_similarity(&emb_a, &emb_b))
}

fn classify_duplicate(
    model_id: &str,
    full_sim: f32,
    title_sim: f32,
    new_title: &str,
    new_desc: &str,
    kept_title: &str,
    kept_desc: &str,
) -> bool {
    let family = duplicate::dedup_encoder_family(model_id);
    let thresholds = thresholds_for_model(model_id);
    if duplicate::titles_match(new_title, kept_title)
        || duplicate::titles_strongly_similar_for_family(family, new_title, kept_title)
        || duplicate::titles_medium_similar_for_family(family, new_title, kept_title)
    {
        return true;
    }
    let distinctive = duplicate::distinctive_common_word_count(new_title, kept_title);
    if distinctive < thresholds.distinctive_common_words {
        return false;
    }
    if matches!(
        family,
        duplicate::DedupEncoderFamily::E5Large | duplicate::DedupEncoderFamily::Bge
    ) && distinctive < 5
    {
        let jaccard = duplicate::distinctive_word_jaccard(new_title, kept_title);
        if jaccard < 0.35 {
            let title_dominant = distinctive >= thresholds.distinctive_high_overlap
                && title_sim >= 0.82
                && full_sim >= 0.68
                && duplicate::has_meaningful_distinctive_overlap(new_title, kept_title);
            if !title_dominant {
                return false;
            }
        }
    }
    if full_sim >= thresholds.full_strong {
        return true;
    }
    if full_sim >= thresholds.full_with_title
        && title_sim >= thresholds.title
        && distinctive >= thresholds.distinctive_min_with_title
    {
        return true;
    }
    if duplicate::titles_similar(new_title, kept_title)
        && distinctive >= thresholds.distinctive_min_titles_similar
        && (full_sim >= thresholds.titles_similar_full
            || title_sim >= thresholds.titles_similar_title)
    {
        return true;
    }
    if distinctive >= thresholds.distinctive_high_overlap
        && full_sim >= thresholds.full_with_lexical
        && title_sim >= thresholds.title - 0.06
    {
        return true;
    }
    if full_sim >= thresholds.full_with_lexical && title_sim >= thresholds.title {
        return true;
    }
    if full_sim >= thresholds.full_with_lexical
        && duplicate::descriptions_similar(new_desc, kept_desc)
    {
        return true;
    }
    false
}

fn analysis_from_scores(
    model_id: &str,
    full_sim: f32,
    title_sim: f32,
    new_title: &str,
    new_desc: &str,
    kept_title: &str,
    kept_desc: &str,
) -> DuplicateAiAnalysis {
    let is_duplicate = classify_duplicate(
        model_id,
        full_sim,
        title_sim,
        new_title,
        new_desc,
        kept_title,
        kept_desc,
    );
    let confidence = (full_sim * 100.0).clamp(0.0, 100.0) as u32;
    let title_pct = (title_sim * 100.0).round() as u32;
    let explanation = if is_duplicate {
        format!(
            "Семантическая близость {confidence}% (заголовки {title_pct}%) — похоже на одну новость"
        )
    } else {
        format!(
            "Семантическая близость {confidence}% (заголовки {title_pct}%) — разные новости"
        )
    };

    DuplicateAiAnalysis {
        is_duplicate,
        confidence,
        explanation,
    }
}

async fn embed_text(
    client: &Client,
    embed_runtime: &LocalEmbedRuntime,
    text: &str,
) -> Result<Vec<f32>> {
    let body = json!({
        "input": text,
        "encoding_format": "float"
    });
    let response = client
        .post(embed_runtime.embeddings_url())
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("embeddings request failed")?
        .error_for_status()
        .context("embeddings API error")?
        .json::<serde_json::Value>()
        .await
        .context("invalid embeddings JSON")?;

    response["data"][0]["embedding"]
        .as_array()
        .context("missing embedding vector")?
        .iter()
        .map(|v| {
            v.as_f64()
                .map(|n| n as f32)
                .context("invalid embedding value")
        })
        .collect()
}

fn prepare_embed_text(model_id: &str, side: EmbedSide, title: &str, description: &str) -> String {
    let base = combined_text(title, description);
    format_embed_input(model_id, side, &base)
}

fn format_embed_input(model_id: &str, side: EmbedSide, text: &str) -> String {
    if model_id.contains("e5") {
        let prefix = match side {
            EmbedSide::Query => "query: ",
            EmbedSide::Passage => "passage: ",
        };
        format!("{prefix}{text}")
    } else if model_id.contains("bge") {
        match side {
            EmbedSide::Query => format!(
                "Represent this sentence for searching relevant passages: {text}"
            ),
            EmbedSide::Passage => text.to_string(),
        }
    } else {
        text.to_string()
    }
}

fn combined_text(title: &str, description: &str) -> String {
    let title = title.trim();
    let description = description.trim();
    if description.is_empty() {
        title.to_string()
    } else if title.is_empty() {
        description.chars().take(512).collect()
    } else {
        format!(
            "{title}. {}",
            description.chars().take(480).collect::<String>()
        )
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.len() != a.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a <= 0.0 || norm_b <= 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_generic_gaming_news_scores() {
        assert!(!classify_duplicate(
            "multilingual-e5-base",
            0.79,
            0.72,
            "Signet City fungalpunk RPG",
            "Play as a mushroom",
            "Games Workshop Emperor reveal",
            "Warhammer 40k lore",
        ));
    }

    #[test]
    fn accepts_strong_full_match() {
        assert!(classify_duplicate(
            "multilingual-e5-base",
            0.88,
            0.75,
            "Super Mario Galaxy Movie Becomes First $1 Billion Film of 2026",
            "box office record",
            "The Super Mario Galaxy Movie crosses $1bn worldwide",
            "Nintendo film success",
        ));
    }

    #[test]
    fn accepts_lexical_assisted_match() {
        assert!(classify_duplicate(
            "multilingual-e5-base",
            0.81,
            0.70,
            "GTA 6 trailer released by Rockstar",
            "Rockstar unveiled the trailer",
            "Rockstar releases GTA 6 trailer",
            "Take-Two announced the trailer",
        ));
    }

    #[test]
    fn accepts_titles_similar_with_moderate_e5_score() {
        assert!(classify_duplicate(
            "multilingual-e5-base",
            0.68,
            0.72,
            "Nintendo Direct Confirmed For June 9, And It's A Big One",
            "Nintendo announced the date",
            "Nintendo Direct Confirmed For June 9, 2026",
            "Direct showcase on June 9",
        ));
    }

    #[test]
    fn accepts_strongly_similar_without_embedding() {
        assert!(classify_duplicate(
            "multilingual-e5-base",
            0.0,
            0.0,
            "Super Mario Galaxy Movie Becomes First $1 Billion Film of 2026",
            "",
            "The Super Mario Galaxy Movie Reaches $1 Billion, And It's The First 2026 Film To Do So",
            "",
        ));
    }

    #[test]
    fn accepts_title_embedding_match_at_76_percent() {
        assert!(classify_duplicate(
            "multilingual-e5-base",
            0.78,
            0.85,
            "Castlevania: Belmont's Curse Gets October 2026 Release Date",
            "Konami announced the date",
            "Konami's next 2D Castlevania game, Belmont's Curse, is arriving in time for Halloween",
            "Halloween 2026 launch",
        ));
    }

    #[test]
    fn rejects_roundup_style_scores_without_title_match() {
        assert!(!classify_duplicate(
            "multilingual-e5-base",
            0.80,
            0.78,
            "Carcass Clad tank game from Mouthwashing team",
            "co-op horror tank sim",
            "The Sunday Papers",
            "Sundays are for recovering after trailerblogging",
        ));
    }

    #[test]
    fn rejects_unrelated_headlines_at_borderline_scores() {
        assert!(!classify_duplicate(
            "multilingual-e5-base",
            0.77,
            0.76,
            "Konami's next 2D Castlevania game, Belmont's Curse, is arriving in time for Halloween",
            "Halloween 2026 launch",
            "Nintendo has confirmed that its next Nintendo Direct will take place tomorrow",
            "Direct showcase tomorrow",
        ));
        assert!(!classify_duplicate(
            "multilingual-e5-base",
            0.72,
            0.75,
            "Bag a huge $308 saving on a two-year ExpressVPN Advanced sub",
            "VPN deal",
            "Nintendo Direct Confirmed For June 9, And It's A Big One",
            "Nintendo announced the date",
        ));
    }

    #[test]
    fn e5_large_accepts_medium_similar_without_high_embedding() {
        assert!(classify_duplicate(
            "multilingual-e5-large",
            0.0,
            0.0,
            "Nintendo Direct Confirmed For June 9, And It's A Big One",
            "",
            "Nintendo Direct Confirmed For June 9, 2026",
            "",
        ));
    }

    #[test]
    fn e5_large_rejects_crazy_taxi_embedding_overlap() {
        assert!(!classify_duplicate(
            "multilingual-e5-large",
            0.90,
            0.84,
            "Not even Crazy Taxi: World Tour is safe from generative AI, as Sega admit they used it",
            "Sega used GenAI for backgrounds",
            "Crazy Taxi: World Tour Dev Defends GenAI Use",
            "developer defends AI use",
        ));
    }

    #[test]
    fn e5_large_accepts_blade_title_dominant_match() {
        assert!(classify_duplicate(
            "multilingual-e5-large",
            0.78,
            0.84,
            "Arkane Lyon's Blade game isn't dead, despite rumours following Xbox Games Showcase no-show",
            "Arkane Lyon Blade rumours",
            "Marvel's Blade Isn't Dead, Despite Recent Rumblings – Report",
            "Marvel Blade still in development",
        ));
    }

    #[test]
    fn e5_large_rejects_weak_overlap_at_borderline_scores() {
        assert!(!classify_duplicate(
            "multilingual-e5-large",
            0.87,
            0.84,
            "XBOX Exclusivity Will Be Decided 'Case-By-Case'",
            "exclusivity strategy",
            "Xbox multiplayer and live service games will still be multiplatform going forward, Matt Booty says",
            "Matt Booty interview",
        ));
        assert!(!classify_duplicate(
            "multilingual-e5-large",
            0.79,
            0.83,
            "The best Amazon Prime Day 2026 gaming headset deals",
            "Prime Day sale",
            "Silent Hill: Townfall Preorders Emerge At Amazon And Best Buy",
            "preorders live",
        ));
        assert!(!classify_duplicate(
            "multilingual-e5-large",
            0.84,
            0.85,
            "Yes, Gears of War: E-Day is bringing back Horde and Versus",
            "multiplayer modes return",
            "Gears of War: E-Day Preorders Are Live for Xbox Fans",
            "preorder editions",
        ));
    }

    #[test]
    fn bge_rejects_weak_overlap_at_borderline_scores() {
        assert!(!classify_duplicate(
            "bge-m3",
            0.88,
            0.85,
            "GTA 6 Gets Its Barbenheimer As Barbie Compilation Sets A November Release",
            "Barbie November release",
            "Only 2 Games Are Brave Enough to Go Up Against the Might of GTA 6 This November",
            "GTA 6 November competition",
        ));
        assert!(!classify_duplicate(
            "bge-m3",
            0.86,
            0.84,
            "Goodbye Pizza Hut, hello Five Guys? Here are the new real-life shops in Crazy Taxi World Tour",
            "Pizza Hut DLC",
            "Crazy Taxi: World Tour Dev Defends GenAI Use",
            "GenAI defense",
        ));
        assert!(!classify_duplicate(
            "bge-m3",
            0.92,
            0.80,
            "Hellblade studio kills its mysterious horrors of the mind project",
            "new Senua game",
            "Ninja Theory experimental horror game Project Mara has been cancelled",
            "Project Mara cancelled",
        ));
    }

    #[test]
    fn bge_accepts_blade_title_dominant_match() {
        assert!(classify_duplicate(
            "bge-m3",
            0.80,
            0.84,
            "Arkane Lyon's Blade game isn't dead, despite rumours following Xbox Games Showcase",
            "Arkane Blade rumours",
            "Marvel's Blade Isn't Dead, Despite Recent Rumblings – Report",
            "Marvel Blade still in development",
        ));
    }

    #[test]
    fn bge_m3_uses_retrieval_instruction_for_query_only() {
        assert_eq!(
            format_embed_input("bge-m3", EmbedSide::Query, "GTA 6 trailer"),
            "Represent this sentence for searching relevant passages: GTA 6 trailer"
        );
        assert_eq!(
            format_embed_input("bge-m3", EmbedSide::Passage, "GTA 6 trailer"),
            "GTA 6 trailer"
        );
    }

    #[test]
    fn e5_large_uses_query_passage_prefixes() {
        assert_eq!(
            format_embed_input("multilingual-e5-large", EmbedSide::Query, "news"),
            "query: news"
        );
        assert_eq!(
            format_embed_input("multilingual-e5-large", EmbedSide::Passage, "news"),
            "passage: news"
        );
    }
}
