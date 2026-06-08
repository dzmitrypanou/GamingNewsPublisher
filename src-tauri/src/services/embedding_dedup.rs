use crate::local_embed_runtime::LocalEmbedRuntime;
use crate::models::{AppSettings, DuplicateAiAnalysis};
use crate::services::duplicate;
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;

/// Strong match alone (e.g. same story, different headline wording).
const FULL_STRONG_THRESHOLD: f32 = 0.88;
/// Needs corroborating title similarity — blocks generic "gaming news" pairs ~0.77–0.83.
const FULL_WITH_TITLE_THRESHOLD: f32 = 0.82;
const TITLE_THRESHOLD: f32 = 0.84;
/// Borderline embedding score plus lexical overlap (rephrased same-language story).
const FULL_WITH_LEXICAL_THRESHOLD: f32 = 0.76;

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
        full_sim,
        title_sim,
        title_a,
        description_a,
        title_b,
        description_b,
    ))
}

/// Compare incoming RSS text against both raw and AI-processed variants of a kept post.
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
    full_sim: f32,
    title_sim: f32,
    new_title: &str,
    new_desc: &str,
    kept_title: &str,
    kept_desc: &str,
) -> bool {
    if full_sim >= FULL_STRONG_THRESHOLD {
        return true;
    }
    if full_sim >= FULL_WITH_TITLE_THRESHOLD && title_sim >= TITLE_THRESHOLD {
        return true;
    }
    if full_sim >= FULL_WITH_LEXICAL_THRESHOLD && title_sim >= TITLE_THRESHOLD {
        return true;
    }
    if full_sim >= FULL_WITH_LEXICAL_THRESHOLD
        && (duplicate::titles_similar(new_title, kept_title)
            || duplicate::descriptions_similar(new_desc, kept_desc))
    {
        return true;
    }
    false
}

fn analysis_from_scores(
    full_sim: f32,
    title_sim: f32,
    new_title: &str,
    new_desc: &str,
    kept_title: &str,
    kept_desc: &str,
) -> DuplicateAiAnalysis {
    let is_duplicate = classify_duplicate(full_sim, title_sim, new_title, new_desc, kept_title, kept_desc);
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
            0.81,
            0.70,
            "GTA 6 trailer released by Rockstar",
            "Rockstar unveiled the trailer",
            "Rockstar releases GTA 6 trailer",
            "Take-Two announced the trailer",
        ));
    }

    #[test]
    fn accepts_title_embedding_match_at_76_percent() {
        assert!(classify_duplicate(
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
            0.80,
            0.78,
            "Carcass Clad tank game from Mouthwashing team",
            "co-op horror tank sim",
            "The Sunday Papers",
            "Sundays are for recovering after trailerblogging",
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
