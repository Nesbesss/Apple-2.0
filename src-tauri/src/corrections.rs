//! Learns word fixes from the user's corrections to dictated text.
//!
//! A fix becomes a vocabulary entry, and from then on it shapes every dictation, so the rules here are
//! strict. A mishearing sounds alike, a rewrite does not. A fix must sit between unchanged words, and a
//! stopword or a short word never teaches anything. Both paths share this code: the text fields of other
//! apps (the helper's `edit` events) and Nesa's own cards (`learn_from_screen`).

use serde::Serialize;

/// The most words the engine wrote for one fix, and the most the user wrote in its place.
const MAX_HEARD_WORDS: usize = 3;
const MAX_MEANT_WORDS: usize = 2;
/// Fewer letters than this is too short to be a word worth learning.
const MIN_LETTERS: usize = 3;
/// The largest share of the letters that may change. Mishearings change a few, rewrites change most.
const MAX_CHANGE: f64 = 0.5;
/// Longer texts are not compared word by word. The comparison is quadratic and a correction is short.
const MAX_TOKENS: usize = 400;
/// Words too common to be a mishearing. A change to one of these is a rephrase, not a fix.
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "for", "from", "i", "if", "in", "is", "it", "its", "me",
    "my", "no", "not", "of", "on", "or", "our", "so", "than", "that", "the", "then", "their", "there", "they", "this",
    "to", "us", "was", "we", "what", "when", "which", "who", "with", "you", "your",
];

/// One learned fix. `heard` is what the engine wrote. It is `None` when only the capitals changed, so the
/// word itself is worth knowing and nothing needs swapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Correction {
    pub heard: Option<String>,
    pub meant: String,
}

/// Whitespace-separated words of a text.
fn tokens(text: &str) -> Vec<&str> {
    text.split_whitespace().collect()
}

/// A word without the punctuation around it. Punctuation inside a word (don't, cooper-netties) stays.
fn core(token: &str) -> &str {
    token.trim_matches(|c: char| !c.is_alphanumeric())
}

/// Letters and digits only, lowercase. Two words are compared on this.
fn letters(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// The number of single-character edits that turn one word into the other.
fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if ca == cb { diagonal } else { 1 + diagonal.min(above).min(row[j]) };
            diagonal = above;
        }
    }
    row[b.len()]
}

/// The pairs of words that match in both texts, in order (a longest common subsequence).
fn unchanged(a: &[&str], b: &[&str]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    let mut table = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[i][j] = if a[i] == b[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut pairs = Vec::new();
    while i < n && j < m {
        if a[i] == b[j] {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if table[i + 1][j] >= table[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

fn is_stopword(word: &str) -> bool {
    STOPWORDS.contains(&core(word).to_lowercase().as_str())
}

/// Decides whether one stretch of changed words is a fix. `heard` is what was there, `meant` what replaced it.
fn correction(heard: &[&str], meant: &[&str]) -> Option<Correction> {
    if heard.is_empty() || meant.is_empty() || heard.len() > MAX_HEARD_WORDS || meant.len() > MAX_MEANT_WORDS {
        return None;
    }
    if heard.iter().chain(meant.iter()).any(|word| is_stopword(word)) {
        return None;
    }
    let heard_text = heard.iter().map(|word| core(word)).collect::<Vec<_>>().join(" ");
    let meant_text = meant.iter().map(|word| core(word)).collect::<Vec<_>>().join(" ");
    let (h, m) = (letters(&heard_text), letters(&meant_text));
    if h.is_empty() || m.is_empty() || h.chars().count() < MIN_LETTERS {
        return None;
    }
    if h == m {
        // The same letters, so only the capitals changed ("claude" to "Claude"). That teaches the word, not a swap.
        let capitalised = heard_text != meant_text && meant_text.chars().any(char::is_uppercase);
        return capitalised.then(|| Correction { heard: None, meant: meant_text });
    }
    let longest = h.chars().count().max(m.chars().count()) as f64;
    if distance(&h, &m) as f64 / longest > MAX_CHANGE {
        return None;
    }
    Some(Correction { heard: Some(heard_text), meant: meant_text })
}

/// The fixes in one edit of a dictated text. Only changes between two unchanged words count: a change at
/// the start or end of the text has nothing to anchor it, so it is never learned. Appended and deleted
/// words are not fixes either.
pub fn learn(inserted: &str, edited: &str) -> Vec<Correction> {
    let (a, b) = (tokens(inserted), tokens(edited));
    if a.is_empty() || b.is_empty() || a.len() > MAX_TOKENS || b.len() > MAX_TOKENS {
        return Vec::new();
    }
    let anchors = unchanged(&a, &b);
    let mut found: Vec<Correction> = Vec::new();
    for pair in anchors.windows(2) {
        let ((ai, bi), (aj, bj)) = (pair[0], pair[1]);
        if let Some(fix) = correction(&a[ai + 1..aj], &b[bi + 1..bj]) {
            if !found.contains(&fix) {
                found.push(fix);
            }
        }
    }
    found
}

/// Whether a character is part of a terminal's box drawing, such as the border of a prompt.
fn is_box(c: char) -> bool {
    ('\u{2500}'..='\u{257F}').contains(&c)
}

/// Finds the dictated text on a screen and returns how it reads now. The text is bounded by its first and
/// last words, which must both still be there. The lowest match wins, so older output above the prompt is
/// skipped. Box drawing and prompt marks are dropped first.
pub fn find_edit(inserted: &str, screen: &str) -> Option<String> {
    let want = tokens(inserted);
    if want.len() < 3 {
        return None;
    }
    let (first, last) = (want[0], want[want.len() - 1]);
    let cleaned: String = screen.chars().map(|c| if is_box(c) { ' ' } else { c }).collect();
    let shown: Vec<&str> = tokens(&cleaned).into_iter().filter(|token| !matches!(*token, ">" | "›" | "❯")).collect();
    let longest = want.len() * 2 + 4;
    for end in (0..shown.len()).rev() {
        if shown[end] != last {
            continue;
        }
        for start in (0..=end).rev() {
            let span = end - start + 1;
            if span > longest {
                break;
            }
            if shown[start] == first && span * 2 >= want.len() {
                return Some(shown[start..=end].join(" "));
            }
        }
    }
    None
}

/// Learns from a Nesa card: the text that was dictated into it, and what the card's prompt shows now.
#[tauri::command]
pub fn learn_from_screen(inserted: String, screen: String) -> Vec<Correction> {
    find_edit(&inserted, &screen).map(|edited| learn(&inserted, &edited)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heard_meant(pairs: &[Correction]) -> Vec<(Option<&str>, &str)> {
        pairs.iter().map(|p| (p.heard.as_deref(), p.meant.as_str())).collect()
    }

    #[test]
    fn a_single_mishearing_is_learned() {
        let fixes = learn("we should use Nessa today", "we should use Nesa today");
        assert_eq!(heard_meant(&fixes), vec![(Some("Nessa"), "Nesa")]);
    }

    #[test]
    fn a_multi_word_mishearing_is_learned_as_one_word() {
        // The example the vocabulary placeholder uses: "cooper netties" is the engine's guess at "Kubernetes".
        let fixes = learn("deploy to cooper netties today", "deploy to Kubernetes today");
        assert_eq!(heard_meant(&fixes), vec![(Some("cooper netties"), "Kubernetes")]);
    }

    #[test]
    fn a_rephrase_is_not_learned() {
        assert!(learn("we should use fast mode now", "we should use quick mode now").is_empty());
    }

    #[test]
    fn a_stopword_swap_is_not_learned() {
        assert!(learn("I think the build is fine", "I think then build is fine").is_empty());
    }

    #[test]
    fn a_deleted_word_is_not_learned() {
        assert!(learn("send it to the team", "send it team").is_empty());
    }

    #[test]
    fn appended_text_is_not_learned() {
        assert!(learn("deploy to prod", "deploy to prod now please").is_empty());
    }

    #[test]
    fn a_change_at_the_edge_has_nothing_to_anchor_it() {
        assert!(learn("Nessa is great", "Nesa is great").is_empty());
        assert!(learn("deploy to Nessa", "deploy to Nesa").is_empty());
    }

    #[test]
    fn punctuation_stuck_to_a_word_does_not_block_a_fix() {
        let fixes = learn("deploy cooper netties, then", "deploy Kubernetes. then");
        assert_eq!(heard_meant(&fixes), vec![(Some("cooper netties"), "Kubernetes")]);
    }

    #[test]
    fn a_repeated_word_teaches_only_the_changed_occurrence() {
        let fixes = learn("send cooper netties then cooper netties", "send Kubernetes then cooper netties");
        assert_eq!(heard_meant(&fixes), vec![(Some("cooper netties"), "Kubernetes")]);
    }

    #[test]
    fn a_capital_fix_teaches_the_word_without_a_swap() {
        let fixes = learn("ping claude now", "ping Claude now");
        assert_eq!(heard_meant(&fixes), vec![(None, "Claude")]);
    }

    #[test]
    fn a_short_word_is_not_learned() {
        // "ok" is two letters and not a stopword, so only the length rule can stop it.
        assert!(learn("the ok thing", "the oh thing").is_empty());
    }

    #[test]
    fn a_single_word_text_has_no_anchors() {
        assert!(learn("Nessa", "Nesa").is_empty());
    }

    #[test]
    fn a_fix_on_screen_is_found_between_its_first_and_last_words() {
        let screen = "│ > deploy to cooper netties today │";
        assert_eq!(find_edit("deploy to cooper netties today", screen).as_deref(), Some("deploy to cooper netties today"));
        let learned = learn_from_screen("deploy to cooper netties today".into(), screen.into());
        assert!(learned.is_empty(), "the screen still has the heard text, so nothing changed");
    }

    #[test]
    fn the_correction_on_screen_is_learned() {
        let screen = "│ > deploy to Kubernetes today │\n│                        │";
        let learned = learn_from_screen("deploy to cooper netties today".into(), screen.into());
        assert_eq!(heard_meant(&learned), vec![(Some("cooper netties"), "Kubernetes")]);
    }

    #[test]
    fn the_lowest_match_on_screen_wins() {
        let screen = "earlier: deploy to cooper netties today\n│ > deploy to Kubernetes today │";
        let edited = find_edit("deploy to cooper netties today", screen);
        assert_eq!(edited.as_deref(), Some("deploy to Kubernetes today"));
    }

    #[test]
    fn a_wrapped_prompt_box_with_a_footer_is_read() {
        // A prompt box that wraps onto a second row, with a footer line below it. The shape is modelled on a
        // terminal prompt, not captured from one.
        let screen = "╭──────────────────────────────────────────────────╮\n\
                      │ > deploy to Kubernetes and then we ship the     │\n\
                      │   build tonight                                  │\n\
                      ╰──────────────────────────────────────────────────╯\n\
                        ? for shortcuts";
        let inserted = "deploy to cooper netties and then we ship the build tonight";
        let learned = learn_from_screen(inserted.into(), screen.into());
        assert_eq!(heard_meant(&learned), vec![(Some("cooper netties"), "Kubernetes")]);
    }

    #[test]
    fn a_screen_without_the_anchors_finds_nothing() {
        assert_eq!(find_edit("deploy to cooper netties today", "nothing like it here"), None);
    }
}
