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
}
