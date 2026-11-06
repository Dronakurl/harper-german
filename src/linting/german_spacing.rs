//! Spacing and comma typography, borrowed from the shared core with German messages.
//!
//! The mistakes are the same in every language written in the Latin script, so the core
//! rules find them. Their messages are English, though, and a German user reads them next
//! to German ones. These wrappers keep the core's spans and suggestions and only translate
//! what they say.

use harper_core::linting::{
    Lint, LintKind, Linter, Suggestion, comma_fixes::CommaFixes, spaces::Spaces,
};
use harper_core::spell::Dictionary;
use harper_core::{Document, Punctuation, Span, TokenKind};

use crate::linting::german_sentence_capitalization::SENTENCE_INTERNAL_ABBREVIATIONS;
use crate::spell::curated_german_dictionary;

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

impl GermanSpaces {
    /// A full stop with no space behind it, between a word and a capitalized
    /// word: *um 10 Uhr.Danach essen wir*, *am 3.Oktober*, *im 19.Jahrhundert*.
    ///
    /// The lexer reads *Uhr.Danach* as one token, a host name, so neither the
    /// spell checker nor the sentence rules ever see the two words. Both halves
    /// have to be dictionary words, the second written with a capital and small
    /// letters after it, so a host name (*wikipedia.de*), a file name
    /// (*bericht.pdf*), a program name (*System.Collections*) and an
    /// abbreviation (*z.B.*, *bzw.Die*) stay out. A number counts as the first
    /// half, which is how dates and ordinals lose their space.
    fn missing_space_after_period(document: &Document) -> Vec<Lint> {
        let source = document.get_source();
        let dictionary = curated_german_dictionary();
        let is_word = |chars: &[char]| {
            dictionary.contains_exact_word(chars)
                || dictionary.contains_exact_word(
                    &chars
                        .iter()
                        .flat_map(|c| c.to_lowercase())
                        .collect::<Vec<_>>(),
                )
        };
        let part_char = |c: char| c.is_alphanumeric();
        let mut lints = Vec::new();

        for (dot, _) in source.iter().enumerate().filter(|(_, c)| **c == '.') {
            let start = (0..dot)
                .rev()
                .take_while(|&i| part_char(source[i]))
                .last()
                .unwrap_or(dot);
            let end = (dot + 1..source.len())
                .take_while(|&i| part_char(source[i]))
                .last()
                .map_or(dot + 1, |i| i + 1);
            let (left, right) = (&source[start..dot], &source[dot + 1..end]);
            if left.is_empty() || right.len() < 3 {
                continue;
            }
            // Something glued to either side makes it an address or a path.
            let glued = |i: Option<&char>| {
                i.is_some_and(|c| matches!(c, '.' | '/' | '@' | '_' | ':' | '-'))
            };
            if glued(start.checked_sub(1).and_then(|i| source.get(i))) || glued(source.get(end)) {
                continue;
            }
            let capitalized =
                right[0].is_uppercase() && right[1..].iter().all(|c| c.is_lowercase());
            if !capitalized || !is_word(right) {
                continue;
            }
            let left_is_number = left.iter().all(char::is_ascii_digit) && left.len() <= 4;
            let left_lower: String = left.iter().flat_map(|c| c.to_lowercase()).collect();
            let left_is_word = left.len() >= 2
                && left.iter().all(|c| c.is_alphabetic())
                && !SENTENCE_INTERNAL_ABBREVIATIONS.contains(&left_lower.as_str())
                && is_word(left);
            if !left_is_number && !left_is_word {
                continue;
            }
            lints.push(Lint {
                span: Span::new(dot, dot + 1),
                lint_kind: LintKind::Formatting,
                suggestions: vec![Suggestion::ReplaceWith(vec!['.', ' '])],
                message: "Nach dem Punkt fehlt ein Leerzeichen.".to_owned(),
                priority: 15,
            });
        }
        lints
    }
}

impl GermanSpaces {
    /// A number glued to the noun it counts: *10Euro*, *5Minuten*. German
    /// writes a space between them (*10 Euro*). Only a capitalized dictionary
    /// word of three letters or more counts as the noun, so units written small
    /// (*10km*, *5kg*), ordinals and versions (*3D*, *5G*, *2er*) stay out.
    fn number_glued_to_noun(document: &Document) -> Vec<Lint> {
        let source = document.get_source();
        let dictionary = curated_german_dictionary();
        let mut lints = Vec::new();
        for at in 1..source.len() {
            if !source[at - 1].is_ascii_digit() || !source[at].is_uppercase() {
                continue;
            }
            let end = (at..source.len())
                .take_while(|&i| source[i].is_alphabetic())
                .last()
                .map_or(at, |i| i + 1);
            let word = &source[at..end];
            let starts_number = (0..at)
                .rev()
                .take_while(|&i| source[i].is_ascii_digit())
                .last()
                .unwrap_or(at - 1);
            let glued_in_front = starts_number
                .checked_sub(1)
                .and_then(|i| source.get(i))
                .is_some_and(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '/' | '.'));
            if word.len() < 3
                || glued_in_front
                || !word[1..].iter().all(|c| c.is_lowercase())
                || !dictionary.contains_exact_word(word)
            {
                continue;
            }
            let spelled: String = word.iter().collect();
            lints.push(Lint {
                span: Span::new(at - 1, at),
                lint_kind: LintKind::Formatting,
                suggestions: vec![Suggestion::InsertAfter(vec![' '])],
                message: format!("Zwischen der Zahl und »{spelled}« steht ein Leerzeichen."),
                priority: 15,
            });
        }
        lints
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
        for lint in Self::gaps_between_sentences(document)
            .into_iter()
            .chain(Self::missing_space_after_period(document))
            .chain(Self::number_glued_to_noun(document))
        {
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

/// Two full stops where one belongs: *finde ich..*, *13,4 %..*. Three are an
/// ellipsis and are left alone, and so is a path (*../datei*).
#[derive(Default)]
pub struct GermanDoublePeriod;

impl Linter for GermanDoublePeriod {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let source = document.get_source();
        (1..source.len().saturating_sub(1))
            .filter(|&at| {
                source[at] == '.'
                    && source[at + 1] == '.'
                    && source[at - 1] != '.'
                    && !source[at - 1].is_whitespace()
                    && source.get(at + 2).is_none_or(|c| c.is_whitespace())
            })
            .map(|at| Lint {
                span: Span::new(at, at + 2),
                lint_kind: LintKind::Punctuation,
                suggestions: vec![Suggestion::ReplaceWith(vec!['.'])],
                message: "Hier steht ein Punkt zu viel.".to_owned(),
                priority: 15,
            })
            .collect()
    }

    fn description(&self) -> &str {
        "Findet zwei Punkte, wo einer genügt."
    }
}

#[cfg(test)]
mod tests {
    use super::{GermanCommaFixes, GermanDoublePeriod, GermanSpaces};
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
    fn reports_a_missing_space_after_a_full_stop() {
        for (text, fixed) in [
            ("Das Treffen ist um 10 Uhr.Danach essen wir.", "Uhr. Danach"),
            ("Am 3.Oktober ist Feiertag.", "3. Oktober"),
            ("Er lebte im 19.Jahrhundert in Wien.", "19. Jahrhundert"),
            ("Ich telefonierte.Ich war müde.", "telefonierte. Ich"),
        ] {
            let lints = lint(text, &mut GermanSpaces::default());
            assert_eq!(lints.len(), 1, "{text}");
            let mut chars: Vec<char> = text.chars().collect();
            lints[0].suggestions[0].apply(lints[0].span, &mut chars);
            let applied: String = chars.into_iter().collect();
            assert!(applied.contains(fixed), "{applied}");
        }
        for text in [
            "Siehe wikipedia.de für mehr.",
            "Die Datei bericht.pdf ist da.",
            "Er nutzt System.Collections im Code.",
            "Er kam z.B. gestern.",
            "Das ist bzw.Das ist anders.",
            "Am 3.10.2024 ist Feiertag.",
            "Schreib an info@firma.Berlin bitte.",
        ] {
            assert!(
                lint(text, &mut GermanSpaces::default()).is_empty(),
                "{text}: {:?}",
                lint(text, &mut GermanSpaces::default())
            );
        }
    }

    #[test]
    fn reports_a_number_glued_to_its_noun() {
        let lints = lint("Das kostet 10Euro.", &mut GermanSpaces::default());
        assert_eq!(lints.len(), 1);
        for text in [
            "Das kostet 10 Euro.",
            "Wir fahren 10km weit.",
            "Ein 3D-Drucker.",
            "Die 5G-Antenne.",
            "Der Code A10Bereich bleibt.",
        ] {
            assert!(
                lint(text, &mut GermanSpaces::default()).is_empty(),
                "{text}"
            );
        }
    }

    #[test]
    fn reports_a_doubled_full_stop() {
        assert_eq!(
            lint("Das ist ärgerlich, finde ich..", &mut GermanDoublePeriod).len(),
            1
        );
        assert_eq!(
            lint("Es lag bei 13,4 %.. Dann fiel es.", &mut GermanDoublePeriod).len(),
            1
        );
        for text in ["Und dann... nichts.", "Siehe ../datei.txt hier.", "Ende."] {
            assert!(lint(text, &mut GermanDoublePeriod).is_empty(), "{text}");
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
