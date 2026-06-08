use crate::services::rss_fetcher::RssItem;
use once_cell::sync::Lazy;
use regex::Regex;

const EXCLUDED_RSS_CATEGORIES: &[&str] = &[
    "multimedia",
    "podcasts",
    "podcast",
    "videos",
    "video",
    "newsletter",
    "puzzles",
];

const NAV_BOILERPLATE_MARKERS: &[&str] = &[
    "topics archive blog columns",
    "interviews podcasts puzzles multimedia",
    "physics mathematics biology computer science topics archive",
    "about quanta contact",
    "all rights reserved quanta",
    "read later bookmark",
    "sign up for quanta",
];

/// Daily puzzle / word-game spoilers (hints, answers, solutions).
const DAILY_PUZZLE_GAMES: &[&str] = &[
    "contexto",
    "wordle",
    "connections",
    "strands",
    "quordle",
    "nerdle",
    "spangram",
    "waffle",
    "worldle",
    "letter-boxed",
    "letter boxed",
    "spelling-bee",
    "spelling bee",
];

const HINTS_ANSWER_PHRASES: &[&str] = &[
    "hints & answer",
    "hints and answer",
    "hints & answers",
    "hints and answers",
    "hint & answer",
    "hint and answer",
    "hints-answer",
    "hints-and-answer",
    "hint-answer",
    "hints-answers",
    "hint-for-",
    "hints-for-",
    "answer-for-",
    "answers-for-",
    "подсказки и ответ",
    "подсказки и ответы",
    "подсказка и ответ",
];

static HINT_WORD: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bhints?\b").expect("hint word"));
static ANSWER_WORD: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\banswers?\b").expect("answer word"));
static SOLUTION_WORD: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bsolutions?\b").expect("solution word"));

/// Title or URL looks like a daily puzzle hints/answers post (Contexto, Wordle, etc.).
pub fn is_hints_or_puzzle_answer_content(title: &str, link: &str) -> bool {
    let title_lower = title.to_ascii_lowercase();
    let url_lower = link.to_ascii_lowercase();
    let title_full_lower = title.to_lowercase();
    let combined = format!("{title_lower} {url_lower}");

    if HINTS_ANSWER_PHRASES
        .iter()
        .any(|phrase| combined.contains(phrase) || title_full_lower.contains(phrase))
    {
        return true;
    }

    if HINT_WORD.is_match(&combined) && ANSWER_WORD.is_match(&combined) {
        return true;
    }

    if title_full_lower.contains("подсказ") && title_full_lower.contains("ответ") {
        return true;
    }

    let has_puzzle = DAILY_PUZZLE_GAMES.iter().any(|game| combined.contains(game));
    if !has_puzzle {
        return false;
    }

    if combined.contains("hints at") || combined.contains("hint at") {
        return false;
    }

    HINT_WORD.is_match(&combined)
        || ANSWER_WORD.is_match(&combined)
        || SOLUTION_WORD.is_match(&combined)
        || combined.contains("today's ")
        || combined.contains("todays-")
        || combined.contains("today-")
}

/// RSS category or URL/type that is not a regular news article.
pub fn is_excluded_rss_category(categories: &[String]) -> bool {
    categories.iter().any(|category| {
        EXCLUDED_RSS_CATEGORIES
            .iter()
            .any(|excluded| category.trim().eq_ignore_ascii_case(excluded))
    })
}

/// Parsed text looks like site navigation instead of article body.
pub fn is_navigation_boilerplate(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return true;
    }

    let lower = trimmed.to_ascii_lowercase();
    if NAV_BOILERPLATE_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return true;
    }

    let words: Vec<&str> = trimmed.split_whitespace().collect();
    let word_count = words.len();
    if word_count < 35 {
        return false;
    }

    let sentence_ends = trimmed
        .chars()
        .filter(|c| matches!(c, '.' | '!' | '?'))
        .count();
    if sentence_ends > 3 {
        return false;
    }

    let menu_like = words
        .iter()
        .filter(|word| {
            word.chars().count() <= 18
                && word
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
        })
        .count();

    menu_like * 100 / word_count.max(1) >= 35
}

pub fn should_exclude_item(item: &RssItem) -> bool {
    should_exclude_content(&item.title, &item.link, &item.description, &item.categories)
}

pub fn should_exclude_content(
    title: &str,
    link: &str,
    description: &str,
    categories: &[String],
) -> bool {
    if is_excluded_rss_category(categories) {
        return true;
    }
    if is_hints_or_puzzle_answer_content(title, link) {
        return true;
    }
    is_navigation_boilerplate(description)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excludes_multimedia_rss_category() {
        assert!(is_excluded_rss_category(&["Multimedia".to_string()]));
        assert!(is_excluded_rss_category(&["Podcasts".to_string()]));
        assert!(!is_excluded_rss_category(&["Biology".to_string()]));
    }

    #[test]
    fn detects_quanta_navigation_soup() {
        let text = "Physics Mathematics Biology Computer Science Topics Archive Blog Columns Interviews Podcasts Puzzles Multimedia Videos About Quanta Contact All Rights Reserved";
        assert!(is_navigation_boilerplate(text));
    }

    #[test]
    fn keeps_normal_article_text() {
        let text = "What is the future of gene editing with CRISPR? Has AI changed mathematics forever? Will we find other civilizations in the universe?";
        assert!(!is_navigation_boilerplate(text));
    }

    #[test]
    fn excludes_item_with_bad_description() {
        let item = RssItem {
            title: "Test".to_string(),
            description: "Physics Mathematics Biology Computer Science Topics Archive Blog Columns".to_string(),
            link: "https://example.com/a".to_string(),
            image_url: None,
            pub_date: None,
            categories: vec!["Biology".to_string()],
        };
        assert!(should_exclude_item(&item));
    }

    #[test]
    fn excludes_contexto_hints_answer_post() {
        assert!(is_hints_or_puzzle_answer_content(
            "Today's Contexto: Hints & Answer For June 8, 2026",
            "https://insider-gaming.com/todays-contexto-hints-answer-for-june-8-2026/",
        ));
    }

    #[test]
    fn excludes_russian_hints_post() {
        assert!(is_hints_or_puzzle_answer_content(
            "Контексто на 8 июня 2026: подсказки и ответ",
            "https://example.com/contexto",
        ));
    }

    #[test]
    fn excludes_wordle_hint_for_today() {
        assert!(is_hints_or_puzzle_answer_content(
            "Today's Wordle Hint For June 8, 2026",
            "https://example.com/todays-wordle-hint-for-june-8-2026/",
        ));
    }

    #[test]
    fn keeps_wordle_hints_at_idiom() {
        assert!(!is_hints_or_puzzle_answer_content(
            "Wordle hints at a future collaboration with Netflix",
            "https://example.com/wordle-netflix-collab",
        ));
    }

    #[test]
    fn keeps_regular_gaming_news() {
        assert!(!is_hints_or_puzzle_answer_content(
            "GTA 6 release date reportedly moved to 2026",
            "https://example.com/gta-6-delay",
        ));
    }
}
