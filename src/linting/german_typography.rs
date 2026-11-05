//! Spacing and comma typography, borrowed from the shared core with German messages.
//!
//! The mistakes are the same in every language written in the Latin script, so the core
//! rules find them. Their messages are English, though, and a German user reads them next
//! to German ones. These wrappers keep the core's spans and suggestions and only translate
//! what they say.

use harper_core::linting::{
    Lint, LintKind, Linter, Suggestion, comma_fixes::CommaFixes, spaces::Spaces,
};
use harper_core::{Document, Punctuation, TokenKind};

/// The core's messages and their German wording. `CommaFixes` may join several of its
/// messages into one, so each sentence is replaced on its own.
const COMMA_MESSAGES: &[(&str, &str)] = &[
    (
        "Don't use a space before a comma.",
        "Vor einem Komma steht kein Leerzeichen.",
    ),
    (
        "Avoid East Asian commas in English contexts.",
        "Im deutschen Text steht ein normales Komma, kein ostasiatisches.",
    ),
    (
        "Use a space after a comma.",
        "Nach einem Komma steht ein Leerzeichen.",
    ),
];

fn translate_comma_message(message: &str) -> String {
    COMMA_MESSAGES
        .iter()
        .fold(message.to_owned(), |message, (english, german)| {
            message.replace(english, german)
        })
}

fn translate_spaces_message(message: &str) -> String {
    if let Some(count) = message
        .strip_prefix("There are ")
        .and_then(|rest| rest.strip_suffix(" spaces where there should be only one."))
    {
        return format!("Hier stehen {count} Leerzeichen, wo eines genügt.");
    }
    if message == "Unnecessary space at the end of the sentence." {
        return "Überflüssiges Leerzeichen vor dem Satzzeichen.".to_owned();
    }
    message.to_owned()
}

/// [`CommaFixes`] with German messages.
#[derive(Default)]
pub struct GermanCommaFixes(CommaFixes);

impl Linter for GermanCommaFixes {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        self.0
            .lint(document)
            .into_iter()
            .map(|lint| Lint {
                message: translate_comma_message(&lint.message),
                ..lint
            })
            .collect()
    }

    fn description(&self) -> &str {
        "Prüft die Leerzeichen um ein Komma: keins davor, eins danach."
    }
}

/// [`Spaces`] with German messages, plus the gap between two sentences.
///
/// The core rule leaves the space in front of a sentence alone, because two spaces after a
/// full stop are a habit English typewriter style allows. German typography (DIN 5008) puts
/// exactly one space there, so this rule reports more than one when it follows a sentence's
/// closing punctuation on the same line.
#[derive(Default)]
pub struct GermanSpaces(Spaces);

impl GermanSpaces {
    fn gaps_between_sentences(document: &Document) -> Vec<Lint> {
        let tokens: Vec<_> = document.tokens().collect();
        tokens
            .windows(3)
            .filter_map(|window| {
                let [before, space, after] = window else {
                    return None;
                };
                let ends_sentence = matches!(
                    before.kind,
                    TokenKind::Punctuation(
                        Punctuation::Period | Punctuation::Bang | Punctuation::Question
                    )
                );
                let TokenKind::Space(count) = space.kind else {
                    return None;
                };
                (ends_sentence && count > 1 && matches!(after.kind, TokenKind::Word(_))).then(
                    || Lint {
                        span: space.span,
                        lint_kind: LintKind::Formatting,
                        suggestions: vec![Suggestion::ReplaceWith(vec![' '])],
                        message: format!(
                            "Zwischen zwei Sätzen steht ein Leerzeichen, hier sind es {count}."
                        ),
                        priority: 15,
                    },
                )
            })
            .collect()
    }
}

impl Linter for GermanSpaces {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints: Vec<Lint> = self
            .0
            .lint(document)
            .into_iter()
            .map(|lint| Lint {
                message: translate_spaces_message(&lint.message),
                ..lint
            })
            .collect();

        // An abbreviation's full stop does not end the sentence, and the core rule has
        // already reported the gap behind it.
        for lint in Self::gaps_between_sentences(document) {
            if !lints.iter().any(|known| known.span == lint.span) {
                lints.push(lint);
            }
        }

        lints
    }

    fn description(&self) -> &str {
        "Zwischen zwei Wörtern und zwischen zwei Sätzen steht genau ein Leerzeichen."
    }
}

#[cfg(test)]
mod tests {
    use super::{GermanCommaFixes, GermanSpaces};
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::{Lint, Linter};

    fn lint(text: &str, linter: &mut impl Linter) -> Vec<Lint> {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        linter.lint(&document)
    }

    #[test]
    fn comma_messages_are_german() {
        let lints = lint(
            "Wir haben Fehler gemacht , weil wir müde waren.",
            &mut GermanCommaFixes::default(),
        );
        assert_eq!(lints.len(), 1);
        assert_eq!(lints[0].message, "Vor einem Komma steht kein Leerzeichen.");

        let lints = lint(
            "Wir kamen,sahen und siegten.",
            &mut GermanCommaFixes::default(),
        );
        assert_eq!(lints.len(), 1);
        assert_eq!(lints[0].message, "Nach einem Komma steht ein Leerzeichen.");
    }

    #[test]
    fn reports_two_spaces_between_sentences() {
        let lints = lint(
            "Das stimmt.  Die Kinder spielen.",
            &mut GermanSpaces::default(),
        );
        assert_eq!(lints.len(), 1);
        assert_eq!(
            lints[0].message,
            "Zwischen zwei Sätzen steht ein Leerzeichen, hier sind es 2."
        );
        assert_eq!(
            lint("Stimmt das?   Ja!  Gut.", &mut GermanSpaces::default()).len(),
            2
        );
    }

    #[test]
    fn reports_two_spaces_between_words_in_german() {
        let lints = lint("Die  Kinder spielen.", &mut GermanSpaces::default());
        assert_eq!(lints.len(), 1);
        assert_eq!(
            lints[0].message,
            "Hier stehen 2 Leerzeichen, wo eines genügt."
        );
    }

    #[test]
    fn allows_single_spaces_and_line_starts() {
        for text in [
            "Das stimmt. Die Kinder spielen.",
            "Das stimmt.\nDie Kinder spielen.",
            "Er kam z. B. gestern.",
            "Ende.",
        ] {
            assert!(
                lint(text, &mut GermanSpaces::default()).is_empty(),
                "{text}"
            );
        }
    }

    #[test]
    fn an_abbreviation_gap_is_reported_once() {
        assert_eq!(
            lint("Er kam z.  B. gestern.", &mut GermanSpaces::default()).len(),
            1
        );
    }
}
