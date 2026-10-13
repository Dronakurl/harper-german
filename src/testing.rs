//! Helpers for the tests in this crate.

use harper_core::Document;
use harper_core::linting::Linter;

use crate::parsers::PlainGerman;
use crate::spell::german_dictionary;

const MAX_DEPTH: usize = 6;

/// Succeeds if applying suggestions, one after another, can turn `text` into `needle`.
#[track_caller]
pub fn assert_suggestion_result(text: &str, mut linter: impl Linter, needle: &str) {
    let dict = german_dictionary();
    let needle: Vec<char> = needle.chars().collect();

    fn search(
        source: Vec<char>,
        linter: &mut impl Linter,
        dict: &impl harper_core::spell::Dictionary,
        needle: &[char],
        depth: usize,
    ) -> bool {
        if source == needle {
            return true;
        }
        if depth == MAX_DEPTH {
            return false;
        }
        let doc = Document::new_from_chars(source.clone().into(), &PlainGerman, dict);
        for lint in linter.lint(&doc) {
            for suggestion in &lint.suggestions {
                let mut next = source.clone();
                suggestion.apply(lint.span, &mut next);
                if search(next, linter, dict, needle, depth + 1) {
                    return true;
                }
            }
        }
        false
    }

    assert!(
        search(text.chars().collect(), &mut linter, &*dict, &needle, 0),
        "No suggestion sequence turned {text:?} into {:?}.",
        needle.iter().collect::<String>()
    );
}
