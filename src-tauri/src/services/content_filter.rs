use crate::services::rss_fetcher::RssItem;

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
    if is_excluded_rss_category(&item.categories) {
        return true;
    }
    is_navigation_boilerplate(&item.description)
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
}
