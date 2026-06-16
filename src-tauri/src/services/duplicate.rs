pub fn normalize_title(title: &str) -> String {
    let lower = title.to_lowercase();
    let cleaned: String = lower
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    cleaned
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn normalize_url(url: &str) -> String {
    let without_query = url.split('?').next().unwrap_or(url);
    let without_fragment = without_query.split('#').next().unwrap_or(without_query);
    without_fragment.trim_end_matches('/').to_lowercase()
}

pub fn titles_match(a: &str, b: &str) -> bool {
    let na = normalize_title(a);
    let nb = normalize_title(b);
    !na.is_empty() && na == nb
}

pub fn normalize_description(description: &str) -> String {
    normalize_title(description)
}

pub fn title_word_jaccard(a: &str, b: &str) -> f64 {
    let norm_a = normalize_title(a);
    let norm_b = normalize_title(b);
    let words_a: std::collections::HashSet<&str> = norm_a
        .split_whitespace()
        .filter(|w| w.len() >= 3)
        .collect();
    let words_b: std::collections::HashSet<&str> = norm_b
        .split_whitespace()
        .filter(|w| w.len() >= 3)
        .collect();
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }
    let common = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    if union == 0 {
        0.0
    } else {
        common as f64 / union as f64
    }
}

const DEDUP_TITLE_STOPWORDS: &[&str] = &[
    "the", "and", "for", "with", "from", "that", "this", "into", "its", "are", "was", "not",
    "announced", "confirmed", "first", "new", "big", "one", "after", "weeks", "about", "how",
    "2026", "2025", "2024", "sale", "discount", "discounts", "game", "games", "video", "film",
];

fn is_dedup_stopword(word: &str) -> bool {
    DEDUP_TITLE_STOPWORDS.contains(&word)
}

fn distinctive_word_set(title: &str) -> std::collections::HashSet<String> {
    normalize_title(title)
        .split_whitespace()
        .filter(|w| w.len() >= 3 && !is_dedup_stopword(w))
        .map(str::to_string)
        .collect()
}

fn distinctive_common_words(a: &str, b: &str) -> usize {
    let words_a = distinctive_word_set(a);
    let words_b = distinctive_word_set(b);
    words_a.intersection(&words_b).count()
}

pub fn distinctive_word_jaccard(a: &str, b: &str) -> f64 {
    let words_a = distinctive_word_set(a);
    let words_b = distinctive_word_set(b);
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }
    let common = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    if union == 0 {
        0.0
    } else {
        common as f64 / union as f64
    }
}

pub fn distinctive_common_word_count(a: &str, b: &str) -> usize {
    distinctive_common_words(a, b)
}

const FRANCHISE_ANCHOR_WORDS: &[&str] = &[
    "world", "tour", "taxi", "crazy", "war", "gears", "day", "game", "live", "service",
];

pub fn has_meaningful_distinctive_overlap(a: &str, b: &str) -> bool {
    let words_a = distinctive_word_set(a);
    let words_b = distinctive_word_set(b);
    let shared: Vec<&str> = words_a
        .intersection(&words_b)
        .map(String::as_str)
        .collect();
    if shared.len() < 4 {
        return true;
    }
    !shared
        .iter()
        .all(|word| FRANCHISE_ANCHOR_WORDS.contains(word))
}

const CONTRADICTORY_DISTINCTIVE_PAIRS: &[(&str, &str)] = &[("summer", "winter")];

fn has_contradictory_distinctive_pair(a: &str, b: &str) -> bool {
    let words_a = distinctive_word_set(a);
    let words_b = distinctive_word_set(b);
    CONTRADICTORY_DISTINCTIVE_PAIRS.iter().any(|(left, right)| {
        words_a.contains(*left) && words_b.contains(*right)
            || words_a.contains(*right) && words_b.contains(*left)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DedupEncoderFamily {
    E5Base,
    E5Large,
    Bge,
    Other,
}

pub fn dedup_encoder_family(model_id: &str) -> DedupEncoderFamily {
    if model_id.contains("e5-large") {
        DedupEncoderFamily::E5Large
    } else if model_id.contains("e5") {
        DedupEncoderFamily::E5Base
    } else if model_id.contains("bge") {
        DedupEncoderFamily::Bge
    } else {
        DedupEncoderFamily::Other
    }
}

fn strongly_similar_for_family(family: DedupEncoderFamily, a: &str, b: &str) -> bool {
    if titles_match(a, b) {
        return true;
    }
    if has_contradictory_distinctive_pair(a, b) {
        return false;
    }
    let common = distinctive_common_words(a, b);
    let jaccard = distinctive_word_jaccard(a, b);
    match family {

        DedupEncoderFamily::E5Base | DedupEncoderFamily::Other => {
            common >= 4 && jaccard >= 0.55
        }

        DedupEncoderFamily::E5Large => common >= 5 || (common >= 4 && jaccard >= 0.45),
        DedupEncoderFamily::Bge => {
            common >= 5
                || (common >= 4 && jaccard >= 0.50 && has_meaningful_distinctive_overlap(a, b))
        }
    }
}

const E5_LARGE_MEDIUM_MIN_JACCARD: f64 = 0.35;

const BGE_MEDIUM_MIN_JACCARD: f64 = 0.30;

pub fn titles_medium_similar_for_family(
    family: DedupEncoderFamily,
    a: &str,
    b: &str,
) -> bool {
    if family != DedupEncoderFamily::E5Large && family != DedupEncoderFamily::Bge {
        return false;
    }
    if titles_match(a, b) || strongly_similar_for_family(family, a, b) {
        return false;
    }
    if has_contradictory_distinctive_pair(a, b) {
        return false;
    }
    let common = distinctive_common_words(a, b);
    let min_jaccard = match family {
        DedupEncoderFamily::E5Large => E5_LARGE_MEDIUM_MIN_JACCARD,
        DedupEncoderFamily::Bge => BGE_MEDIUM_MIN_JACCARD,
        _ => 1.0,
    };
    titles_similar(a, b) && common >= 3 && distinctive_word_jaccard(a, b) >= min_jaccard
}

pub fn duplicate_match_rank_for_family(
    family: DedupEncoderFamily,
    new_title: &str,
    kept_title: &str,
    confidence: u32,
) -> (u32, u32, u32) {
    let lexical = if titles_match(new_title, kept_title) {
        3
    } else if strongly_similar_for_family(family, new_title, kept_title) {
        3
    } else if titles_medium_similar_for_family(family, new_title, kept_title) {
        2
    } else if titles_similar(new_title, kept_title) {
        1
    } else {
        0
    };
    (
        lexical,
        distinctive_common_words(new_title, kept_title) as u32,
        confidence,
    )
}

pub fn duplicate_match_rank(new_title: &str, kept_title: &str, confidence: u32) -> (u32, u32, u32) {
    duplicate_match_rank_for_family(DedupEncoderFamily::E5Base, new_title, kept_title, confidence)
}

pub fn titles_strongly_similar_for_family(family: DedupEncoderFamily, a: &str, b: &str) -> bool {
    strongly_similar_for_family(family, a, b)
}

pub fn titles_strongly_similar(a: &str, b: &str) -> bool {
    strongly_similar_for_family(DedupEncoderFamily::E5Base, a, b)
}

pub fn titles_similar(a: &str, b: &str) -> bool {
    if titles_match(a, b) {
        return true;
    }

    let na = normalize_title(a);
    let nb = normalize_title(b);
    if na.is_empty() || nb.is_empty() {
        return false;
    }

    let (shorter, longer) = if na.len() <= nb.len() {
        (&na, &nb)
    } else {
        (&nb, &na)
    };

    if shorter.len() >= 24 && longer.contains(shorter.as_str()) {
        return true;
    }

    let words_a: Vec<&str> = na
        .split_whitespace()
        .filter(|w| w.len() >= 3)
        .collect();
    let words_b: Vec<&str> = nb
        .split_whitespace()
        .filter(|w| w.len() >= 3)
        .collect();

    if words_a.is_empty() || words_b.is_empty() {
        return false;
    }

    let common = words_a
        .iter()
        .filter(|w| words_b.contains(w))
        .count();
    let min_words = words_a.len().min(words_b.len());
    if common >= 2 && common as f64 / min_words as f64 >= 0.5 {
        return true;
    }

    let union = words_a.len() + words_b.len() - common;
    if union == 0 {
        return false;
    }

    common as f64 / union as f64 >= 0.65
}

pub fn descriptions_similar(a: &str, b: &str) -> bool {
    let na: String = normalize_description(a).chars().take(220).collect();
    let nb: String = normalize_description(b).chars().take(220).collect();

    if na.len() < 60 || nb.len() < 60 {
        return false;
    }

    titles_match(&na, &nb) || titles_similar(&na, &nb)
}

pub fn content_matches(
    new_title: &str,
    new_desc: &str,
    existing_title: &str,
    existing_desc: &str,
) -> bool {
    titles_similar(new_title, existing_title) || descriptions_similar(new_desc, existing_desc)
}

pub fn is_link_roundup_title(title: &str) -> bool {
    let norm = normalize_title(title);
    if norm.is_empty() {
        return false;
    }

    const EXACT: &[&str] = &[
        "the sunday papers",
        "sunday papers",
        "saturday critic",
        "friday roundup",
        "weekly roundup",
        "news roundup",
        "news round up",
        "this week in games",
        "week in review",
        "the week in review",
        "link dump",
        "links we liked",
        "morning coffee",
        "the morning coffee",
        "critical distance",
        "free games roundup",
    ];

    if EXACT.iter().any(|p| norm == *p) {
        return true;
    }

    if norm.contains("sunday paper")
        || norm.contains("roundup")
        || norm.contains("round up")
        || norm.starts_with("weekly digest")
        || norm.starts_with("daily digest")
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_title_strips_punctuation() {
        assert_eq!(
            normalize_title("Game Announced — Official Trailer!"),
            "game announced official trailer"
        );
    }

    #[test]
    fn normalize_url_strips_tracking() {
        assert_eq!(
            normalize_url("https://Example.com/news/?utm_source=rss"),
            "https://example.com/news"
        );
    }

    #[test]
    fn titles_similar_for_same_story() {
        assert!(titles_similar(
            "GTA 6 trailer released by Rockstar",
            "Rockstar releases GTA 6 trailer"
        ));
    }

    #[test]
    fn detects_link_roundup_titles() {
        assert!(is_link_roundup_title("The Sunday Papers"));
        assert!(is_link_roundup_title("Sunday Papers"));
        assert!(is_link_roundup_title("Weekly Roundup: Best RPG news"));
        assert!(!is_link_roundup_title(
            "Castlevania: Belmont's Curse Gets October 2026 Release Date"
        ));
    }

    #[test]
    fn strongly_similar_catches_reworded_headlines() {
        assert!(titles_strongly_similar(
            "The Super Mario Galaxy Movie Reaches $1 Billion, And It's The First 2026 Film To Do So",
            "Super Mario Galaxy Movie Becomes First $1 Billion Film of 2026",
        ));
        assert!(titles_strongly_similar(
            "Xbox confirms console exclusivity will be decided on \"case by case basis\"",
            "XBOX Exclusivity Will Be Decided 'Case-By-Case'",
        ));
    }

    #[test]
    fn strongly_similar_rejects_different_sale_seasons() {
        assert!(!titles_strongly_similar(
            "Steam Summer Sale 2026 announced with big discounts",
            "Steam Winter Sale 2026 announced with big discounts",
        ));
    }

    #[test]
    fn strongly_similar_rejects_same_franchise_different_story() {
        assert!(!titles_strongly_similar(
            "Not even Crazy Taxi: World Tour is safe from generative AI, as Sega admit they used it",
            "Goodbye Pizza Hut, hello Five Guys? Here's some of the new real-life shops in Crazy Taxi World Tour",
        ));
    }

    #[test]
    fn distinctive_word_jaccard_differs_from_full_title_jaccard() {
        let jaccard = distinctive_word_jaccard(
            "Not even Crazy Taxi: World Tour is safe from generative AI",
            "Goodbye Pizza Hut, hello Five Guys in Crazy Taxi World Tour",
        );
        assert!(jaccard < 0.45);
    }

    #[test]
    fn e5_large_catches_long_mario_headlines() {
        assert!(titles_strongly_similar_for_family(
            DedupEncoderFamily::E5Large,
            "The Super Mario Galaxy Movie Reaches $1 Billion, And It's The First 2026 Film To Do So",
            "The Super Mario Galaxy Movie has finally passed $1 billion globally, making it the first 2026 film",
        ));
    }

    #[test]
    fn e5_large_medium_similar_for_nintendo_direct() {
        assert!(titles_medium_similar_for_family(
            DedupEncoderFamily::E5Large,
            "Nintendo Direct Confirmed For June 9, And It's A Big One",
            "Nintendo Direct Confirmed For June 9, 2026",
        ));
        assert!(!titles_medium_similar_for_family(
            DedupEncoderFamily::E5Base,
            "Nintendo Direct Confirmed For June 9, And It's A Big One",
            "Nintendo Direct Confirmed For June 9, 2026",
        ));
    }

    #[test]
    fn e5_large_medium_rejects_franchise_only_overlap() {
        assert!(!titles_medium_similar_for_family(
            DedupEncoderFamily::E5Large,
            "Not even Crazy Taxi: World Tour is safe from generative AI, as Sega admit they used it",
            "Crazy Taxi: World Tour Dev Defends GenAI Use",
        ));
    }

    #[test]
    fn e5_large_strong_rejects_crazy_taxi_franchise_overlap() {
        assert!(!titles_strongly_similar_for_family(
            DedupEncoderFamily::E5Large,
            "Not even Crazy Taxi: World Tour is safe from generative AI, as Sega admit they used it",
            "Crazy Taxi: World Tour Dev Defends GenAI Use",
        ));
    }

    #[test]
    fn bge_medium_similar_for_nintendo_direct() {
        assert!(titles_medium_similar_for_family(
            DedupEncoderFamily::Bge,
            "Nintendo Direct Confirmed For June 9, And It's A Big One",
            "Nintendo Direct Confirmed For June 9, 2026",
        ));
    }

    #[test]
    fn meaningful_overlap_rejects_franchise_only_shared_words() {
        assert!(!has_meaningful_distinctive_overlap(
            "Not even Crazy Taxi: World Tour is safe from generative AI",
            "Crazy Taxi: World Tour Dev Defends GenAI Use",
        ));
        assert!(has_meaningful_distinctive_overlap(
            "Arkane Lyon's Blade game isn't dead, despite rumours",
            "Marvel's Blade Isn't Dead, Despite Recent Rumblings",
        ));
    }
}
