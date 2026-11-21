use crate::models::ModDbSummary;

struct ParsedQuery {
    norm: String,
    compact: String,
    tokens: Vec<String>,
}

impl ParsedQuery {
    fn new(raw: &str) -> Self {
        Self {
            norm: normalize_text(raw),
            compact: compact_text(raw),
            tokens: tokenize(raw),
        }
    }

    fn is_empty(&self) -> bool {
        self.compact.is_empty() && self.tokens.is_empty()
    }
}

pub fn normalize_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_space = true;
    for ch in input.chars() {
        if ch.is_alphanumeric() {
            for lc in ch.to_lowercase() {
                out.push(lc);
            }
            prev_space = false;
        } else if !prev_space {
            out.push(' ');
            prev_space = true;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

pub fn compact_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_alphanumeric() {
            for lc in ch.to_lowercase() {
                out.push(lc);
            }
        }
    }
    out
}

pub fn tokenize(input: &str) -> Vec<String> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut prev_kind: Option<CharKind> = None;

    for (i, &ch) in chars.iter().enumerate() {
        if !ch.is_alphanumeric() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            prev_kind = None;
            continue;
        }

        let kind = if ch.is_numeric() {
            CharKind::Digit
        } else if ch.is_uppercase() {
            CharKind::Upper
        } else {
            CharKind::Lower
        };

        let next_is_lower = chars.get(i + 1).is_some_and(|c| c.is_lowercase());
        let boundary = matches!(
            (prev_kind, kind),
            (Some(CharKind::Lower), CharKind::Upper)
                | (Some(CharKind::Lower | CharKind::Upper), CharKind::Digit)
                | (Some(CharKind::Digit), CharKind::Lower | CharKind::Upper)
        ) || (matches!(prev_kind, Some(CharKind::Upper))
            && matches!(kind, CharKind::Upper)
            && next_is_lower);

        if boundary && !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }

        for lc in ch.to_lowercase() {
            current.push(lc);
        }
        prev_kind = Some(kind);
    }

    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[derive(Copy, Clone)]
enum CharKind {
    Lower,
    Upper,
    Digit,
}

fn max_edit_distance(token_char_len: usize) -> usize {
    if token_char_len <= 3 {
        0
    } else if token_char_len <= 6 {
        1
    } else {
        2
    }
}

/// Optimal string alignment (Damerau-Levenshtein) distance bounded by `max_dist`.
/// Returns `None` if the distance exceeds `max_dist`.
pub fn bounded_damerau_levenshtein(a: &str, b: &str, max_dist: usize) -> Option<usize> {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    if m.abs_diff(n) > max_dist {
        return None;
    }
    if max_dist == 0 {
        return if a_chars == b_chars { Some(0) } else { None };
    }
    if m == 0 {
        return Some(n);
    }
    if n == 0 {
        return Some(m);
    }

    let inf = max_dist + 1;
    let cols = n + 1;
    let mut dp = vec![inf; (m + 1) * cols];

    for i in 0..=m {
        dp[i * cols] = i;
    }
    for j in 0..=n {
        dp[j] = j;
    }

    for i in 1..=m {
        let mut row_min = inf;
        for j in 1..=n {
            let cost = usize::from(a_chars[i - 1] != b_chars[j - 1]);
            let mut best = (dp[(i - 1) * cols + j] + 1)
                .min(dp[i * cols + (j - 1)] + 1)
                .min(dp[(i - 1) * cols + (j - 1)] + cost);

            if i > 1
                && j > 1
                && a_chars[i - 1] == b_chars[j - 2]
                && a_chars[i - 2] == b_chars[j - 1]
            {
                best = best.min(dp[(i - 2) * cols + (j - 2)] + 1);
            }

            dp[i * cols + j] = best;
            row_min = row_min.min(best);
        }
        if row_min > max_dist {
            return None;
        }
    }

    let dist = dp[m * cols + n];
    if dist <= max_dist {
        Some(dist)
    } else {
        None
    }
}

enum TokenMatch {
    Exact(usize),
    Prefix(usize),
    Fuzzy(usize, usize), // (token_idx, edit_distance)
}

impl TokenMatch {
    fn idx(&self) -> usize {
        match *self {
            Self::Exact(i) | Self::Prefix(i) | Self::Fuzzy(i, _) => i,
        }
    }

    fn penalty(&self) -> i64 {
        match *self {
            Self::Exact(_) => 0,
            Self::Prefix(_) => 1,
            Self::Fuzzy(_, dist) => (dist as i64) * 10,
        }
    }
}

fn match_all_tokens(query_tokens: &[String], target_tokens: &[String], allow_fuzzy: bool) -> Option<(i64, bool)> {
    if query_tokens.is_empty() || target_tokens.is_empty() {
        return None;
    }

    let mut total_penalty = 0i64;
    let mut matched_indices = Vec::with_capacity(query_tokens.len());

    for qt in query_tokens {
        let mut best: Option<TokenMatch> = None;

        for (idx, tt) in target_tokens.iter().enumerate() {
            if tt == qt {
                best = Some(TokenMatch::Exact(idx));
                break;
            }
            if qt.len() >= 2 && tt.starts_with(qt.as_str()) {
                if !matches!(best, Some(TokenMatch::Exact(_)) | Some(TokenMatch::Prefix(_))) {
                    best = Some(TokenMatch::Prefix(idx));
                }
                continue;
            }
            if allow_fuzzy {
                let max_d = max_edit_distance(qt.chars().count());
                if max_d > 0 {
                    if let Some(d) = bounded_damerau_levenshtein(qt, tt, max_d) {
                        let replace = match &best {
                            None => true,
                            Some(TokenMatch::Fuzzy(_, prev_d)) => d < *prev_d,
                            Some(_) => false,
                        };
                        if replace {
                            best = Some(TokenMatch::Fuzzy(idx, d));
                        }
                    }
                }
            }
        }

        let m = best?;
        total_penalty += m.penalty();
        matched_indices.push(m.idx());
    }

    let in_order = matched_indices.windows(2).all(|w| w[0] < w[1]);
    Some((total_penalty, in_order))
}

fn score_field_text(
    text: &str,
    q: &ParsedQuery,
    exact_score: i64,
    starts_score: i64,
    contains_score: i64,
    tokens_base_score: i64,
    allow_fuzzy: bool,
) -> i64 {
    if text.trim().is_empty() {
        return 0;
    }

    let norm = normalize_text(text);
    let compact = compact_text(text);

    if norm == q.norm {
        return exact_score;
    }
    if compact == q.compact {
        return exact_score - 300;
    }
    if !q.norm.is_empty() && norm.starts_with(&q.norm) {
        return starts_score;
    }
    if !q.compact.is_empty() && compact.starts_with(&q.compact) {
        return starts_score - 300;
    }
    if !q.norm.is_empty() && norm.contains(&q.norm) {
        return contains_score;
    }
    if !q.compact.is_empty() && compact.contains(&q.compact) {
        return contains_score - 300;
    }

    let target_tokens = tokenize(text);
    if let Some((penalty, in_order)) = match_all_tokens(&q.tokens, &target_tokens, allow_fuzzy) {
        let order_bonus = if in_order { 250 } else { 0 };
        return (tokens_base_score - penalty * 120 + order_bonus).max(1);
    }

    // Handle merged-word typos like "combtoverhaul" against ["combat", "overhaul", "fork"]
    if allow_fuzzy && q.tokens.len() == 1 && q.compact.chars().count() >= 6 && target_tokens.len() >= 2 {
        let max_d = if q.compact.chars().count() >= 10 { 2 } else { 1 };
        let mut best_dist: Option<usize> = None;
        for window_size in 2..=3.min(target_tokens.len()) {
            for win in target_tokens.windows(window_size) {
                let joined = win.concat();
                if let Some(d) = bounded_damerau_levenshtein(&q.compact, &joined, max_d) {
                    best_dist = Some(best_dist.map_or(d, |prev| prev.min(d)));
                }
            }
        }
        if let Some(d) = best_dist {
            return (tokens_base_score - 1200 - (d as i64) * 200).max(1);
        }
    }

    0
}

fn score_mod_parsed(item: &ModDbSummary, q: &ParsedQuery) -> i64 {
    let mut name_score = score_field_text(&item.name, q, 10_000, 8_200, 6_800, 5_500, true);
    if name_score > 0 {
        let name_token_count = tokenize(&item.name).len() as i64;
        name_score += 150_i64.saturating_sub(name_token_count * 15);
    }

    let mut id_score = 0i64;
    for id_str in &item.modidstrs {
        id_score = id_score.max(score_field_text(
            id_str,
            q,
            4_500,
            4_100,
            3_600,
            3_000,
            true,
        ));
    }
    if let Some(alias) = &item.urlalias {
        id_score = id_score.max(score_field_text(
            alias,
            q,
            4_500,
            4_100,
            3_600,
            3_000,
            true,
        ));
    }

    let mut meta_score = 0i64;
    if let Some(author) = &item.author {
        meta_score = meta_score.max(score_field_text(
            author,
            q,
            2_000,
            1_800,
            1_600,
            1_400,
            true,
        ));
    }
    for tag in &item.tags {
        meta_score = meta_score.max(score_field_text(
            tag,
            q,
            1_900,
            1_700,
            1_500,
            1_300,
            false,
        ));
    }

    let summary_score = item
        .summary
        .as_deref()
        .map(|s| score_field_text(s, q, 1_000, 950, 900, 600, false))
        .unwrap_or(0);

    let best = name_score.max(id_score).max(meta_score).max(summary_score);
    if best == 0 {
        return 0;
    }

    // Small boost when multiple fields match, kept strictly smaller than a tier step (500).
    let secondary_bonus = i64::from(name_score > 0 && summary_score > 0) * 50
        + i64::from(name_score > 0 && id_score > 0) * 80;
    best + secondary_bonus
}

pub fn filter_and_rank_mods(
    mods: &[ModDbSummary],
    raw_query: &str,
    order_by: Option<&str>,
) -> Vec<ModDbSummary> {
    let q = ParsedQuery::new(raw_query);
    if q.is_empty() {
        return mods.to_vec();
    }

    let mut scored: Vec<(i64, ModDbSummary)> = mods
        .iter()
        .filter_map(|m| {
            let score = score_mod_parsed(m, &q);
            if score > 0 {
                Some((score, m.clone()))
            } else {
                None
            }
        })
        .collect();

    let order = order_by.unwrap_or("trendingpoints").trim().to_lowercase();
    scored.sort_by(|(sa, a), (sb, b)| {
        // Bucket scores into relevance tiers so popularity/order_by breaks ties within a tier
        // while strong title matches always stay above summary-only matches.
        let tier_a = sa / 500;
        let tier_b = sb / 500;
        tier_b
            .cmp(&tier_a)
            .then_with(|| compare_by_order(a, b, &order))
            .then_with(|| sb.cmp(sa))
    });

    scored.into_iter().map(|(_, m)| m).collect()
}

fn compare_by_order(a: &ModDbSummary, b: &ModDbSummary, order: &str) -> std::cmp::Ordering {
    match order {
        "downloads" => b
            .downloads
            .unwrap_or(0)
            .cmp(&a.downloads.unwrap_or(0))
            .then_with(|| b.trendingpoints.unwrap_or(0).cmp(&a.trendingpoints.unwrap_or(0))),
        "follows" => b
            .follows
            .unwrap_or(0)
            .cmp(&a.follows.unwrap_or(0))
            .then_with(|| b.downloads.unwrap_or(0).cmp(&a.downloads.unwrap_or(0))),
        "lastreleased" => b
            .lastreleased
            .as_deref()
            .unwrap_or("")
            .cmp(a.lastreleased.as_deref().unwrap_or(""))
            .then_with(|| b.downloads.unwrap_or(0).cmp(&a.downloads.unwrap_or(0))),
        _ => b
            .trendingpoints
            .unwrap_or(0)
            .cmp(&a.trendingpoints.unwrap_or(0))
            .then_with(|| b.downloads.unwrap_or(0).cmp(&a.downloads.unwrap_or(0))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mod(modid: i64, name: &str, summary: &str, downloads: i64, trending: i64) -> ModDbSummary {
        ModDbSummary {
            modid,
            assetid: None,
            downloads: Some(downloads),
            follows: Some(downloads / 10),
            trendingpoints: Some(trending),
            comments: None,
            name: name.to_string(),
            summary: Some(summary.to_string()),
            modidstrs: vec![],
            author: Some("Author".to_string()),
            urlalias: None,
            side: Some("both".to_string()),
            mod_type: Some("mod".to_string()),
            logo: None,
            tags: vec![],
            lastreleased: Some("2026-01-01".to_string()),
        }
    }

    #[test]
    fn combat_overhaul_ranks_fork_above_summary_mentions() {
        let mods = vec![
            make_mod(2568, "BetterChinese", "Adds Combat Overhaul translations", 14118, 6),
            make_mod(8415, "Combat Overhaul Fork", "Fork of Combat Overhaul", 143666, 0),
            make_mod(10635, "SLCombatOverhaul", "Addon for Combat Overhaul", 28, 0),
            make_mod(3607, "Combat Overhaul: Reanimated", "Reanimation mod", 35132, 1),
            make_mod(9999, "Unrelated Farming Mod", "Grow wheat and turnips", 50000, 10),
        ];

        let results = filter_and_rank_mods(&mods, "combat overhaul", Some("trendingpoints"));
        let names: Vec<&str> = results.iter().map(|m| m.name.as_str()).collect();
        assert!(!names.contains(&"Unrelated Farming Mod"));
        assert!(names[..2].contains(&"Combat Overhaul Fork"));
        assert!(names[..2].contains(&"Combat Overhaul: Reanimated"));
        assert_eq!(names.last().copied(), Some("BetterChinese"));

        let by_downloads = filter_and_rank_mods(&mods, "combat overhaul", Some("downloads"));
        assert_eq!(by_downloads[0].name, "Combat Overhaul Fork");
    }

    #[test]
    fn matches_without_spaces_or_with_reordered_words_or_typos() {
        let mods = vec![
            make_mod(8415, "Combat Overhaul Fork", "Fork of Combat Overhaul", 143666, 0),
            make_mod(10635, "SLCombatOverhaul", "Addon", 28, 0),
        ];

        assert_eq!(
            filter_and_rank_mods(&mods, "combatoverhaul", None)[0].modid,
            8415
        );
        assert_eq!(
            filter_and_rank_mods(&mods, "fork combat", None)[0].modid,
            8415
        );
        assert_eq!(
            filter_and_rank_mods(&mods, "combt overhal", None)[0].modid,
            8415
        );
        assert_eq!(
            filter_and_rank_mods(&mods, "combtoverhaul", None)[0].modid,
            8415
        );
    }

    #[test]
    fn tokenize_splits_camel_case_and_punctuation() {
        assert_eq!(
            tokenize("SLCombatOverhaul-v1"),
            vec!["sl", "combat", "overhaul", "v", "1"]
        );
    }
}
