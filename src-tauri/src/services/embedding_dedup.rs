use crate::local_embed_runtime::LocalEmbedRuntime;
use crate::models::{AppSettings, DuplicateAiAnalysis};
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;

const DUPLICATE_THRESHOLD: f32 = 0.82;

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

    let text_a = combined_text(title_a, description_a);
    let text_b = combined_text(title_b, description_b);
    let (emb_a, emb_b) = tokio::try_join!(
        embed_text(client, embed_runtime, &text_a),
        embed_text(client, embed_runtime, &text_b),
    )?;

    let similarity = cosine_similarity(&emb_a, &emb_b);
    let is_duplicate = similarity >= DUPLICATE_THRESHOLD;
    let confidence = (similarity * 100.0).clamp(0.0, 100.0) as u32;
    let explanation = if is_duplicate {
        format!(
            "Семантическая близость {confidence}% — похоже на одну новость (порог {DUPLICATE_THRESHOLD:.0}%)"
        )
    } else {
        format!("Семантическая близость {confidence}% — разные новости")
    };

    Ok(DuplicateAiAnalysis {
        is_duplicate,
        confidence,
        explanation,
    })
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
