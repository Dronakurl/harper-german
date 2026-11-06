use std::sync::LazyLock;

use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::spell::Dictionary;
use harper_core::{Document, TokenStringExt};
use hashbrown::{HashMap, HashSet};

use crate::spell::curated_german_dictionary;

/// Misspellings written down by hand from everyday writing, which no slip of
/// the fingers explains. The ones with a space in the correction are adverbs
/// written closed up: *schon mal* and *nicht mal* are two words, while
/// *nochmal* is a word of its own and stays out. Sorted by UTF-8 bytes, so
/// `binary_search_by_key` can find them.
const HAND_PICKED_TYPOS: &[(&str, &str)] = &[
    ("bischen", "bisschen"),
    ("nichtmal", "nicht mal"),
    ("nocheinmal", "noch einmal"),
    ("packet", "Paket"),
    ("schoneinmal", "schon einmal"),
    ("schonmal", "schon mal"),
    ("wiedereinmal", "wieder einmal"),
    ("wiedermal", "wieder mal"),
];

/// The frequent words whose slips are looked for; see `data/frequent_words.txt`.
const FREQUENT_WORDS: &str = include_str!("../../data/frequent_words.txt");

/// The shortest slip worth a table entry: below it, a slip of one word is too
/// often another word.
const MIN_SLIP_LEN: usize = 5;

/// Every slip of the fingers on a frequent word that is no word itself, with
/// the word it was meant to be, keyed in lower case: one letter left out, two
/// neighbours swapped, one letter struck twice. The first letter is never
/// touched, and the last is never left out, because a word without its last
/// letter is mostly another form of it (*Verbunde*, *Schul-*). A slip that two
/// words share says nothing and is dropped.
static FINGER_SLIPS: LazyLock<HashMap<String, &'static str>> = LazyLock::new(|| {
    let base = curated_german_dictionary();
    let words: Vec<&'static str> = FREQUENT_WORDS
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    let frequent: HashSet<String> = words.iter().map(|word| word.to_lowercase()).collect();
    let mut slips: HashMap<String, &'static str> = HashMap::new();
    let mut shared: HashSet<String> = HashSet::new();
    for word in &words {
        let lower: Vec<char> = word.to_lowercase().chars().collect();
        let mut variants: Vec<Vec<char>> = Vec::new();
        for at in 1..lower.len() {
            if at + 1 < lower.len() {
                let mut left_out = lower.clone();
                left_out.remove(at);
                variants.push(left_out);
                let mut swapped = lower.clone();
                swapped.swap(at, at + 1);
                variants.push(swapped);
            }
            let mut doubled = lower.clone();
            doubled.insert(at, lower[at]);
            variants.push(doubled);
        }
        for variant in variants {
            let key: String = variant.iter().collect();
            if variant.len() < MIN_SLIP_LEN
                || variant == lower
                || frequent.contains(&key)
                || base.contains_word(&variant)
            {
                continue;
            }
            match slips.get(&key) {
                Some(other) if !other.eq_ignore_ascii_case(word) => {
                    shared.insert(key);
                }
                _ => {
                    slips.insert(key, word);
                }
            }
        }
    }
    for key in shared {
        slips.remove(&key);
    }
    slips
});

/// Corrects the misspellings people actually make, which the spell checker
/// lets through.
///
/// German compounds freely, so Harper's spell checker accepts any word it can
/// cut into dictionary pieces. That is what makes German spell checking work
/// at all, and it is also why ordinary typos of the commonest words sail past
/// it: *Beisiel* cuts into `Bei` + `Siel`, *Zusand* into `zu` + `Sand`,
/// *Antort* into `an` + `Tort`. Every one of them is a legal cut of illegal
/// German.
///
/// The slips are not listed but made: [`FINGER_SLIPS`] types every word of a
/// hand-written list of frequent words the ways fingers slip, and keeps what
/// the dictionary does not know. The few misspellings no slip explains are
/// [`HAND_PICKED_TYPOS`].
///
/// Case is the writer's: the suggestion copies the casing of what was typed,
/// so `austellung` becomes `ausstellung` and is left to
/// [`super::GermanNounCapitalization`] to capitalize. Fixing both here would
/// make one lint out of two independent mistakes.
#[derive(Default)]
pub struct GermanCommonTypos;

impl GermanCommonTypos {
    fn correction(word: &[char]) -> Option<&'static str> {
        let lower: String = word.iter().flat_map(|c| c.to_lowercase()).collect();
        HAND_PICKED_TYPOS
            .binary_search_by_key(&lower.as_str(), |(wrong, _)| wrong)
            .ok()
            .map(|index| HAND_PICKED_TYPOS[index].1)
            .or_else(|| FINGER_SLIPS.get(&lower).copied())
    }
}

impl GermanCommonTypos {
    /// The correction of a compound that *opens* with a known misspelling:
    /// *Beisielsatz* is *Beispiel* + *Satz*, and the compound splitter accepts
    /// it as readily as it accepts *Beisiel* on its own.
    ///
    /// Only openings of five letters or more count, and the rest has to be a
    /// dictionary word of at least three letters by itself, written small as
    /// the compound writes it or capitalized as the noun is. Five letters keep
    /// short slips from matching the start of a word that merely begins the
    /// same way. Linking elements are not tried: a writer who knows the seam
    /// of *Beispielsatz* knows the word.
    fn compound_correction(word: &[char]) -> Option<String> {
        let lower: Vec<char> = word.iter().flat_map(|c| c.to_lowercase()).collect();
        let dictionary = curated_german_dictionary();
        // The longest opening that is a typo, with three letters left behind it.
        let (length, right) = (MIN_SLIP_LEN..=lower.len().saturating_sub(3))
            .rev()
            .find_map(|length| {
                Self::correction(&lower[..length])
                    .filter(|right| !right.contains(' '))
                    .map(|right| (length, right))
            })?;
        // The opening may merely be how a correct compound begins: *Kenntnis* +
        // *Stand* starts with *kenntniss*, *hoch* + *ausgebildeten* with
        // *hochaus*. When the word splits into two dictionary words at a seam
        // inside the opening, it is that compound and not the typo.
        let text: String = lower.iter().collect();
        if Self::splits_inside(&text, length) {
            return None;
        }
        let rest = lower[length..].to_vec();
        let mut capitalized = rest.clone();
        capitalized[0] = capitalized[0].to_uppercase().next()?;
        let is_word = |w: &[char]| {
            dictionary.contains_exact_word(w)
                && dictionary
                    .get_word_metadata(w)
                    .is_some_and(|m| !m.is_proper_noun())
        };
        (is_word(&rest) || is_word(&capitalized)).then(|| {
            let rest: String = rest.into_iter().collect();
            format!("{}{rest}", right.to_lowercase())
        })
    }
}

impl GermanCommonTypos {
    /// Does `word` split into two dictionary words, with or without a linking
    /// element, at a seam before character `limit`?
    fn splits_inside(word: &str, limit: usize) -> bool {
        let dictionary = curated_german_dictionary();
        let chars: Vec<char> = word.chars().collect();
        let is_word = |part: &[char]| {
            let mut capitalized = part.to_vec();
            if let Some(first) = capitalized.first_mut() {
                *first = first.to_uppercase().next().unwrap_or(*first);
            }
            part.len() >= 3
                && (dictionary.contains_exact_word(part)
                    || dictionary.contains_exact_word(&capitalized))
        };
        (3..limit.min(chars.len())).any(|seam| {
            let (head, rest) = chars.split_at(seam);
            is_word(head)
                && ["", "s", "n", "en", "es", "er", "e"].iter().any(|link| {
                    let link: Vec<char> = link.chars().collect();
                    rest.strip_prefix(link.as_slice()).is_some_and(is_word)
                })
        })
    }
}

impl Linter for GermanCommonTypos {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();

        let base = curated_german_dictionary();
        for word in document.iter_words() {
            let chars = document.get_span_content(&word.span);
            // A word of the dictionary is no slip, whatever it opens with:
            // *theoretisch* is not *Theorie* misspelled plus *tisch*. The
            // hand-picked misspellings are words to it (*bischen*, *Packet*).
            let hand_picked = {
                let lower: String = chars.iter().flat_map(|c| c.to_lowercase()).collect();
                HAND_PICKED_TYPOS
                    .binary_search_by_key(&lower.as_str(), |(wrong, _)| wrong)
                    .is_ok()
            };
            if !hand_picked && base.contains_word(chars) {
                continue;
            }
            let (correction, in_compound) = match Self::correction(chars) {
                Some(correction) => (correction.to_string(), false),
                None => match Self::compound_correction(chars) {
                    Some(correction) => (correction, true),
                    None => continue,
                },
            };

            let original: String = chars.iter().collect();
            let message = if in_compound {
                format!("»{original}« beginnt mit einem häufigen Tippfehler.")
            } else if correction.contains(' ') {
                format!("»{correction}« schreibt man getrennt.")
            } else {
                format!("»{original}« ist ein häufiger Tippfehler.")
            };
            lints.push(Lint {
                span: word.span,
                lint_kind: LintKind::Spelling,
                suggestions: vec![Suggestion::replace_with_match_case_str(&correction, chars)],
                message,
                priority: 19,
            });
        }

        lints
    }

    fn description(&self) -> &str {
        "Korrigiert häufige deutsche Tippfehler, die der Rechtschreibprüfung entgehen."
    }
}

#[cfg(test)]
mod tests {
    use super::{FINGER_SLIPS, GermanCommonTypos, HAND_PICKED_TYPOS};
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn document(text: &str) -> Document {
        Document::new(text, &PlainGerman, &combined_german_dictionary())
    }

    fn flagged(text: &str) -> Vec<String> {
        let document = document(text);
        GermanCommonTypos
            .lint(&document)
            .into_iter()
            .map(|lint| document.get_span_content_str(&lint.span))
            .collect()
    }

    fn correction(word: &str) -> Option<&'static str> {
        let chars: Vec<char> = word.chars().collect();
        GermanCommonTypos::correction(&chars)
    }

    /// `binary_search_by_key` compares `&str` by UTF-8 bytes. A table sorted
    /// any other way — by code point after NFD, or by a locale that files `ü`
    /// under `u` — still looks orderly and silently stops finding its umlaut
    /// entries.
    #[test]
    fn the_table_is_in_the_order_the_search_assumes() {
        for pair in HAND_PICKED_TYPOS.windows(2) {
            assert!(
                pair[0].0 < pair[1].0,
                "{:?} must sort before {:?}",
                pair[0].0,
                pair[1].0
            );
        }
    }

    /// A correction that is itself a key would leave the writer fixing the
    /// same word forever.
    #[test]
    fn no_correction_is_itself_a_typo() {
        for (wrong, right) in HAND_PICKED_TYPOS {
            assert_eq!(correction(right), None, "{wrong} -> {right} loops");
        }
        for (wrong, right) in FINGER_SLIPS.iter() {
            assert_eq!(correction(right), None, "{wrong} -> {right} loops");
        }
    }

    #[test]
    fn catches_the_typos_the_compound_splitter_waves_through() {
        for (wrong, right) in [
            ("Beisiel", "Beispiel"),
            ("Zusand", "Zustand"),
            ("Antort", "Antwort"),
            ("Eigenschafen", "Eigenschaften"),
            ("allerings", "allerdings"),
            ("Gegensand", "Gegenstand"),
            ("Grundalge", "Grundlage"),
            ("gesammten", "gesamten"),
        ] {
            assert_eq!(correction(wrong), Some(right), "{wrong}");
            assert_eq!(
                flagged(&format!("Hier steht {wrong} im Satz.")),
                vec![wrong]
            );
        }
    }

    #[test]
    fn catches_a_typo_at_the_start_of_a_compound() {
        for (text, right) in [
            ("Das ist ein Beisielsatz.", "Beispielsatz"),
            ("Die Antortschreiben kamen.", "Antwortschreiben"),
        ] {
            let document = document(text);
            let lints = GermanCommonTypos.lint(&document);
            assert_eq!(lints.len(), 1, "{text}");
            assert!(
                format!("{:?}", lints[0].suggestions).contains(right),
                "{text}: {:?}",
                lints[0].suggestions
            );
        }
        for text in [
            "Das ist ein Kunststoffeimer.",
            "Die Kunst offenbart sich.",
            "Nach heutigem Kenntnisstand ist das so.",
            "Sie sind hochausgebildete Fachkräfte.",
            "Reize gehen auf die aufsteigende Bahn.",
        ] {
            assert!(flagged(text).is_empty(), "{text}");
        }
    }

    #[test]
    fn splits_adverbs_written_closed_up() {
        for (text, wrong, right) in [
            ("Ich habe es dir schonmal gesagt.", "schonmal", "schon mal"),
            ("Das ist nichtmal schlimm.", "nichtmal", "nicht mal"),
            ("Er hat das wiedermal vergessen.", "wiedermal", "wieder mal"),
            (
                "Ich bin wiedereinmal zu spät.",
                "wiedereinmal",
                "wieder einmal",
            ),
            ("Schonmal davon gehört?", "Schonmal", "Schon mal"),
            ("Das ist ein bischen viel.", "bischen", "bisschen"),
            ("Wir haben das Packet bekommen.", "Packet", "Paket"),
        ] {
            let document = document(text);
            let lints = GermanCommonTypos.lint(&document);
            assert_eq!(lints.len(), 1, "{text}");
            assert_eq!(document.get_span_content_str(&lints[0].span), wrong);
            assert!(
                format!("{:?}", lints[0].suggestions).contains(right),
                "{text}: {:?}",
                lints[0].suggestions
            );
        }
        for text in [
            "Sag es noch mal.",
            "Sag es nochmal.",
            "Ich war schon mal da.",
        ] {
            assert!(flagged(text).is_empty(), "{text}");
        }
    }

    /// The suggestion takes the writer's casing, not the table's. Capitalizing
    /// a lower-case noun is a second, separate mistake and belongs to
    /// `GermanNounCapitalization`.
    #[test]
    fn the_suggestion_follows_the_writers_case() {
        let document = document("Die beisiel und die Beisiel.");
        let lints = GermanCommonTypos.lint(&document);

        let suggestions: Vec<String> = lints
            .iter()
            .flat_map(|lint| &lint.suggestions)
            .map(|suggestion| format!("{suggestion:?}"))
            .collect();

        assert_eq!(lints.len(), 2);
        assert!(suggestions[0].contains("beispiel"), "{suggestions:?}");
        assert!(suggestions[1].contains("Beispiel"), "{suggestions:?}");
    }

    /// Real words, some of them one slip from a frequent word, stay out.
    #[test]
    fn leaves_real_german_words_alone() {
        for word in [
            "anderseits",
            "wohlgesonnen",
            "Nachkommens",
            "Autorenschaft",
            "Hilfsfonds",
            "Amtssitz",
            "Bundesstraße",
            "vielleicht",
            "Mannschaft",
            "Aufnahme",
            "Kenntnis",
            "Standard",
            "Rückgrat",
            "Verbunde",
            "Schul",
            "Antworten",
            "Beispiele",
        ] {
            assert_eq!(correction(word), None, "{word} should be left alone");
        }
    }
}
