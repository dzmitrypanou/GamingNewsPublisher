use regex::Regex;
use std::sync::OnceLock;

const SENTENCES_PER_PARAGRAPH: usize = 2;

fn url_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(https?://\S+|www\.\S+)").expect("url regex"))
}

pub fn contains_url(text: &str) -> bool {
    url_regex().is_match(text)
}

pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn ends_sentence(token: &str) -> bool {
    token
        .chars()
        .last()
        .is_some_and(|ch| matches!(ch, '.' | '!' | '?' | '…'))
}

fn split_into_segments(text: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();

    for token in text.split_whitespace() {
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(token);

        if ends_sentence(token) {
            segments.push(current.trim().to_string());
            current.clear();
        }
    }

    if !current.trim().is_empty() {
        segments.push(current.trim().to_string());
    }

    segments
}

fn split_paragraphs(text: &str) -> Vec<String> {
    if text.contains("\n\n") {
        text.split("\n\n")
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect()
    } else if text.contains('\n') {
        text.split('\n')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect()
    } else {
        vec![text.trim().to_string()]
    }
}

fn strip_link_sentences_in_block(text: &str) -> String {
    split_into_segments(text)
        .into_iter()
        .filter(|part| !should_drop_sentence(part))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn should_drop_sentence(text: &str) -> bool {
    contains_url(text) || is_cta_sentence(text)
}

fn normalize_cta_text(text: &str) -> String {
    text.trim()
        .trim_matches('*')
        .trim()
        .to_lowercase()
}

fn is_cta_sentence(text: &str) -> bool {
    let lower = normalize_cta_text(text);
    if lower.is_empty() {
        return false;
    }

    const MARKERS: &[&str] = &[
        "подробнее в статье",
        "подробности в статье",
        "читайте полностью",
        "читать полностью",
        "читайте в источнике",
        "подробности по ссылке",
        "следите за календар",
        "следите за релиз",
        "следите за новост",
        "следите за обновлен",
        "чтобы ничего не пропустить",
        "чтобы не пропустить",
        "не пропустите релиз",
        "не пропустите выход",
        "осталось следить за",
        "следим за календар",
        "read more",
        "read the full",
        "read the article",
        "details in the article",
        "more in the article",
        "more details in the article",
        "see the full article",
        "full story",
        "link in bio",
        "click here to read",
        "continue reading",
        "stay tuned",
        "don't miss",
        "don’t miss",
        "keep an eye on",
        "follow for updates",
        "watch for updates",
    ];

    if MARKERS.iter().any(|marker| lower.contains(marker)) {
        return true;
    }

    cta_regexes().iter().any(|re| re.is_match(&lower))
}

fn cta_regexes() -> &'static [Regex] {
    static RES: OnceLock<Vec<Regex>> = OnceLock::new();
    RES.get_or_init(|| {
        [
            r"следите за\b.{0,80}\b(?:чтобы )?ничего не пропустить",
            r"следите за\b.{0,80}\b(?:релиз|календар|анонс|обновлен)",
            r"(?:не )?пропустите\b.{0,60}\b(?:релиз|выход|анонс|новост)",
            r"keep an eye on\b.{0,80}\b(?:release )?calendar",
        ]
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect()
    })
}

pub fn strip_links_single_line(text: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }

    normalize_inline_whitespace(&strip_link_sentences_in_block(text))
}

pub fn format_post_text(text: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }

    let stripped: Vec<String> = split_paragraphs(text)
        .into_iter()
        .map(|paragraph| strip_link_sentences_in_block(&paragraph))
        .filter(|paragraph| !paragraph.is_empty())
        .collect();

    if stripped.is_empty() {
        return String::new();
    }

    if stripped.len() > 1 {
        return stripped.join("\n\n");
    }

    ensure_paragraphs_in_block(&stripped[0])
}

fn ensure_paragraphs_in_block(text: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }

    let sentences = split_into_segments(text);
    if sentences.len() <= SENTENCES_PER_PARAGRAPH {
        return sentences.join(" ");
    }

    sentences
        .chunks(SENTENCES_PER_PARAGRAPH)
        .map(|chunk| chunk.join(" "))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn normalize_inline_whitespace(text: &str) -> String {
    static SPACE_RE: OnceLock<Regex> = OnceLock::new();
    let re = SPACE_RE.get_or_init(|| Regex::new(r"[ \t]+").expect("space regex"));
    re.replace_all(text.trim(), " ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_sentence_with_youtube_link() {
        let input = "Текст о игре. Смотрите трейлер: https://youtu.be/abc123";
        let output = format_post_text(input);
        assert_eq!(output, "Текст о игре.");
    }

    #[test]
    fn removes_middle_sentence_with_url() {
        let input = "Первое предложение. Ссылка https://example.com/path. Третье предложение.";
        let output = format_post_text(input);
        assert_eq!(output, "Первое предложение. Третье предложение.");
    }

    #[test]
    fn keeps_text_without_links() {
        let input = "Только обычный текст без ссылок.";
        assert_eq!(format_post_text(input), input);
    }

    #[test]
    fn splits_long_text_into_paragraphs() {
        let input = "Первое. Второе. Третье. Четвёртое.";
        let output = format_post_text(input);
        assert_eq!(output, "Первое. Второе.\n\nТретье. Четвёртое.");
    }

    #[test]
    fn preserves_existing_paragraphs() {
        let input = "Абзац один. Ещё предложение.\n\nАбзац два. И снова текст.";
        let output = format_post_text(input);
        assert_eq!(output, input);
    }

    #[test]
    fn removes_www_link_sentence() {
        let input = "Подробности на www.ign.com/article";
        assert_eq!(format_post_text(input), "");
    }

    #[test]
    fn removes_read_more_in_article_cta() {
        let input = "Производство выросло в мае. Подробнее в статье.";
        assert_eq!(
            format_post_text(input),
            "Производство выросло в мае."
        );
    }

    #[test]
    fn removes_release_calendar_cta() {
        let input = "Осенью выйдет много игр, включая GTA 6. Следите за календарем релизов, чтобы ничего не пропустить.";
        assert_eq!(
            format_post_text(input),
            "Осенью выйдет много игр, включая GTA 6."
        );
    }
}
