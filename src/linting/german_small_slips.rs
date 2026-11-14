//! Small forms of spelling that need only the characters around them: *rief:„Halt!“*,
//! *3-Zimmer Wohnung*, *18 jährig*, *auf deutsch*, *Er sagte: „wir kommen“*.

use harper_core::{
    Document, Span,
    linting::{Lint, LintKind, Linter, Suggestion},
};

const LANGUAGES: &[&str] = &[
    "deutsch",
    "englisch",
    "französisch",
    "spanisch",
    "italienisch",
    "russisch",
    "türkisch",
    "polnisch",
    "arabisch",
    "chinesisch",
    "japanisch",
    "portugiesisch",
    "niederländisch",
];

const SPEECH_OPENERS: &[&str] = &[
    "wir", "ich", "du", "er", "sie", "es", "ihr", "man", "das", "was", "wo", "wie", "heute",
    "morgen", "bitte", "danke", "nein", "ja", "komm", "kommt", "lass",
];

/// Reports typographic and capitalization slips found from the characters alone:
///
/// * a colon right in front of an opening quotation mark: *rief:„Halt!“* → *rief: „Halt!“*;
/// * the first word of direct speech in lower case behind *: „*;
/// * *3-Zimmer Wohnung* → *3-Zimmer-Wohnung*, *18 jährig* → *18-jährig*;
/// * a language after *auf* in lower case: *auf deutsch* → *auf Deutsch*.
#[derive(Default)]
pub struct GermanSmallSlips;

fn is_letter(c: char) -> bool {
    c.is_alphabetic()
}

impl Linter for GermanSmallSlips {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let source = document.get_full_content();
        let mut lints = Vec::new();
        let word_at = |start: usize| -> (usize, String) {
            let end = source[start..]
                .iter()
                .position(|c| !is_letter(*c))
                .map_or(source.len(), |n| start + n);
            (end, source[start..end].iter().collect())
        };
        let mut at = 0;
        while at < source.len() {
            let c = source[at];
            // *rief:„Halt!“*
            if c == ':' && at > 0 && is_letter(source[at - 1]) && source.get(at + 1) == Some(&'„')
            {
                lints.push(Lint {
                    span: Span::new(at, at + 1),
                    lint_kind: LintKind::Formatting,
                    suggestions: vec![Suggestion::ReplaceWith(vec![':', ' '])],
                    message: "Nach dem Doppelpunkt steht ein Leerzeichen.".to_owned(),
                    priority: 31,
                });
            }
            // *Er sagte: „wir kommen“*
            if c == '„' && at >= 1 {
                let before = if source[at - 1] == ' ' && at >= 2 {
                    source[at - 2]
                } else {
                    source[at - 1]
                };
                if before == ':' && source.get(at + 1).is_some_and(|c| c.is_lowercase()) {
                    let (end, word) = word_at(at + 1);
                    if SPEECH_OPENERS.contains(&word.as_str()) {
                        let first: String = word.chars().next().unwrap().to_uppercase().collect();
                        let fixed = format!(
                            "{first}{}",
                            &word[word.chars().next().unwrap().len_utf8()..]
                        );
                        lints.push(Lint {
                            span: Span::new(at + 1, end),
                            lint_kind: LintKind::Capitalization,
                            suggestions: vec![Suggestion::ReplaceWith(fixed.chars().collect())],
                            message: "Direkte Rede beginnt mit einem Großbuchstaben.".to_owned(),
                            priority: 31,
                        });
                    }
                }
            }
            // *3-Zimmer Wohnung*
            if c.is_ascii_digit() && (at == 0 || !source[at - 1].is_alphanumeric()) {
                let mut end = at;
                while source.get(end).is_some_and(|c| c.is_ascii_digit()) {
                    end += 1;
                }
                if source.get(end) == Some(&'-') {
                    let (word_end, word) = word_at(end + 1);
                    if word.chars().count() >= 4
                        && word.chars().next().is_some_and(char::is_uppercase)
                        && source.get(word_end) == Some(&' ')
                        && source.get(word_end + 1).is_some_and(|c| c.is_uppercase())
                        && !word.chars().all(char::is_uppercase)
                        && word.chars().skip(1).all(char::is_lowercase)
                    {
                        lints.push(Lint {
                            span: Span::new(word_end, word_end + 1),
                            lint_kind: LintKind::Formatting,
                            suggestions: vec![Suggestion::ReplaceWith(vec!['-'])],
                            message: "Eine Zusammensetzung mit Zahl wird durchgekoppelt: »3-Zimmer-Wohnung«."
                                .to_owned(),
                            priority: 31,
                        });
                    }
                } else if source.get(end) == Some(&' ') {
                    // *18 jährig*
                    let (_, word) = word_at(end + 1);
                    if word.ends_with("jährig") && word.chars().count() <= 12 {
                        lints.push(Lint {
                            span: Span::new(end, end + 1),
                            lint_kind: LintKind::Formatting,
                            suggestions: vec![Suggestion::ReplaceWith(vec!['-'])],
                            message:
                                "Zahl und »-jährig« werden mit Bindestrich verbunden: »18-jährig«."
                                    .to_owned(),
                            priority: 31,
                        });
                    }
                }
                at = end.max(at + 1);
                continue;
            }
            // *auf deutsch*
            if (c == 'a' || c == 'A')
                && (at == 0 || !is_letter(source[at - 1]))
                && source[at..].starts_with(&['a', 'u', 'f', ' '])
            {
                let (end, word) = word_at(at + 4);
                if LANGUAGES.contains(&word.as_str()) {
                    let first: String = word.chars().next().unwrap().to_uppercase().collect();
                    let fixed = format!(
                        "{first}{}",
                        &word[word.chars().next().unwrap().len_utf8()..]
                    );
                    lints.push(Lint {
                        span: Span::new(at + 4, end),
                        lint_kind: LintKind::Capitalization,
                        suggestions: vec![Suggestion::ReplaceWith(fixed.chars().collect())],
                        message: "Die Sprache nach »auf« ist ein Nomen: »auf Deutsch«.".to_owned(),
                        priority: 31,
                    });
                }
            }
            at += 1;
        }
        lints
    }

    fn description(&self) -> &str {
        "Findet kleine Schreibfehler an den Zeichen: Leerzeichen nach dem Doppelpunkt, Durchkopplung, »auf Deutsch«."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanSmallSlips;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn count(text: &str) -> usize {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanSmallSlips.lint(&document).len()
    }

    #[test]
    fn small_slips_are_reported() {
        for text in [
            "Er rief:„Halt!“",
            "Er sagte: „wir kommen morgen“.",
            "Das ist ein 3-Zimmer Wohnung.",
            "Er ist 18 jährig.",
            "Er spricht auf deutsch mit mir.",
        ] {
            assert_eq!(count(text), 1, "{text}");
        }
    }

    #[test]
    fn right_forms_are_quiet() {
        for text in [
            "Er rief: „Halt!“",
            "Er sagte: „Wir kommen morgen“.",
            "Das ist eine 3-Zimmer-Wohnung.",
            "Er ist 18-jährig.",
            "Er spricht auf Deutsch mit mir.",
            "Er nannte es: „kleiner Trick“.",
            "Das Haus hat 3 Zimmer Wohnung.",
            "Es gibt 18 Jahre.",
            "Das ist ein 5-Kilo Paket.",
        ] {
            let expected = usize::from(text.contains("5-Kilo"));
            assert_eq!(count(text), expected, "{text}");
        }
    }
}
