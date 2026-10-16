//! Is a text German?
//!
//! The same signals as the detector this was moved from, built on three things that need no
//! dictionary:
//! - German special characters (ä, ö, ü, ß): very high confidence,
//! - how many words are common German function words,
//! - how many words are common English function words, which rules English out.
//!
//! The original compared words with the English dictionary. A plugin does not carry one, so a
//! list of English function words stands in; its thresholds are scaled to match.

use harper_core::parsers::Parser;
use harper_core::{Token, TokenKind};

use crate::parsers::PlainGerman;

/// Words so common in German that a text with many of them is German.
const GERMAN_WORDS: &[&str] = &[
    "der", "die", "das", "den", "dem", "des", "ein", "eine", "einer", "einen", "einem", "ich",
    "du", "er", "sie", "es", "wir", "ihr", "mich", "dich", "ihn", "uns", "euch", "ist", "sind",
    "war", "waren", "hat", "habe", "haben", "hatte", "hatten", "werden", "wird", "wurde", "worden",
    "kann", "kannst", "können", "könnte", "machen", "macht", "machte", "gehen", "geht", "ging",
    "gingen", "im", "am", "um", "für", "durch", "während", "seit", "von", "zu", "bei", "dass",
    "dies", "diese", "dieser", "dieses", "jener", "jene", "jenes", "nicht", "nichts", "kein",
    "keine", "keiner", "keinen", "auch", "noch", "schon", "nur", "doch", "ja", "nein", "oder",
    "und",
];

/// English function words that are not also German words (`in`, `an`, `was`, `so`, `man` are).
const ENGLISH_WORDS: &[&str] = &[
    "the", "and", "of", "to", "is", "it", "that", "for", "with", "as", "are", "be", "this", "by",
    "from", "or", "have", "not", "but", "they", "you", "we", "he", "she", "his", "her", "their",
    "which", "on", "at", "were", "been", "has", "had", "would", "there", "what", "when", "your",
    "can", "will", "all", "if", "my", "about", "into", "than", "then", "its", "who", "how",
];

/// How sure we are that `text` is German, from 0 to 1, or `None` if there is nothing to go on
/// (too short, or clearly something else).
pub fn german_confidence(text: &str) -> Option<f32> {
    let chars: Vec<char> = text.chars().collect();
    german_confidence_of_tokens(&PlainGerman.parse(&chars), &chars)
}

/// [`german_confidence`] when the host says how much of `text` is English: `english_ratio` is the
/// share of its words in Harper's English dictionary. These are the thresholds of the detector
/// this was moved from, which had that dictionary at hand.
pub fn german_confidence_given_english(text: &str, english_ratio: f32) -> Option<f32> {
    let chars: Vec<char> = text.chars().collect();
    let counts = Counts::of(&PlainGerman.parse(&chars), &chars)?;

    if counts.ratio(counts.special) >= 0.01 {
        return Some(0.95);
    }
    // English is clearly dominant.
    if english_ratio >= 0.65 {
        return None;
    }
    if counts.ratio(counts.german) >= 0.20 {
        return Some(0.85);
    }
    if english_ratio < 0.4 && counts.ratio(counts.german) >= 0.08 {
        return Some(0.6);
    }
    None
}

/// What the confidence is computed from.
struct Counts {
    total: usize,
    special: usize,
    german: usize,
    english: usize,
}

impl Counts {
    /// `None` when there are too few words to judge reliably.
    fn of(tokens: &[Token], source: &[char]) -> Option<Self> {
        let mut counts = Self {
            total: 0,
            special: 0,
            german: 0,
            english: 0,
        };
        for token in tokens {
            if !matches!(token.kind, TokenKind::Word(_)) {
                continue;
            }
            counts.total += 1;
            let word: String = token
                .get_ch(source)
                .iter()
                .collect::<String>()
                .to_lowercase();
            // ß is German only; ä, ö and ü also occur in other languages, but together they count.
            if word.contains(['ä', 'ö', 'ü', 'ß']) {
                counts.special += 1;
            }
            if GERMAN_WORDS.contains(&word.as_str()) {
                counts.german += 1;
            }
            if ENGLISH_WORDS.contains(&word.as_str()) {
                counts.english += 1;
            }
        }
        (counts.total >= 5).then_some(counts)
    }

    fn ratio(&self, n: usize) -> f32 {
        n as f32 / self.total as f32
    }
}

fn german_confidence_of_tokens(tokens: &[Token], source: &[char]) -> Option<f32> {
    let counts = Counts::of(tokens, source)?;

    if counts.ratio(counts.special) >= 0.01 {
        return Some(0.95);
    }
    // English is clearly dominant.
    if counts.ratio(counts.english) >= 0.25 {
        return None;
    }
    if counts.ratio(counts.german) >= 0.20 {
        return Some(0.85);
    }
    if counts.ratio(counts.english) < 0.10 && counts.ratio(counts.german) >= 0.08 {
        return Some(0.6);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::german_confidence;

    fn is_german(text: &str) -> bool {
        german_confidence(text).is_some()
    }

    #[test]
    fn detects_german_special_chars() {
        assert!(is_german(
            "Der Hund spielt im Garten mit Äpfeln und Ölkannen."
        ));
    }

    #[test]
    fn detects_common_german_words() {
        assert!(is_german(
            "Der Hund ist im Garten. Die Katze schläft auf dem Sofa. Das Auto ist sehr schnell."
        ));
    }

    #[test]
    fn detects_mixed_german_english() {
        assert!(is_german(
            "Der Hund plays im Garten. die Katze sleeps auf dem Sofa."
        ));
    }

    #[test]
    fn rejects_english() {
        assert!(!is_german(
            "The dog plays in the garden. The cat sleeps on the sofa. The car is very fast."
        ));
    }

    #[test]
    fn rejects_short_text() {
        assert!(!is_german("Der Hund"));
    }

    #[test]
    fn detects_longer_german_text() {
        assert!(is_german(
            "Das ist eine Anleitung für den Gebrauch der Maschine. \
             Der Hund spielt im Garten und die Katze schläft auf dem Sofa. \
             Das Auto ist sehr schnell und der Vogel singt im Baum. \
             Wir gehen heute ins Kino und essen danach im Restaurant."
        ));
    }

    #[test]
    fn english_with_a_german_word_stays_english() {
        assert!(!is_german(
            "We went to the old town and she said that the Kindergarten was closed for the day."
        ));
    }

    #[test]
    fn code_is_not_german() {
        assert!(!is_german("fn main() { let x = 5; println!(\"{}\", x); }"));
    }

    #[test]
    fn the_sample_texts_are_german() {
        for name in [
            "german_basic.md",
            "german_technical.md",
            "german_support_ticket.md",
        ] {
            let text = std::fs::read_to_string(format!("test_sources/{name}")).unwrap();
            assert!(is_german(&text), "{name}");
        }
    }

    /// An English sentence about German grammar, with a few German words in it. The English
    /// dictionary knows most of its words, so it is not German.
    #[test]
    fn english_about_german_is_english_when_the_host_says_so() {
        use super::german_confidence_given_english;
        let text = "sentence — die + masculine/neuter noun as a plural subject, a dative in \
                    subject position, die before an umlaut-plural singular";
        assert_eq!(german_confidence_given_english(text, 0.8), None);
        assert!(
            german_confidence_given_english("Der Hund ist im Garten und spielt.", 0.1).is_some()
        );
    }
}
