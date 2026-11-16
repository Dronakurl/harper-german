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
            // *Bei fragen*: *Fragen* is a noun.
            if (c == 'b' || c == 'B')
                && (at == 0 || !is_letter(source[at - 1]))
                && source[at + 1..].starts_with(&['e', 'i', ' '][..])
            {
                let (end, word) = word_at(at + 4);
                if word == "fragen" || word == "problemen" || word == "rückfragen" {
                    let first: String = word.chars().next().unwrap().to_uppercase().collect();
                    let fixed = format!(
                        "{first}{}",
                        &word[word.chars().next().unwrap().len_utf8()..]
                    );
                    lints.push(Lint {
                        span: Span::new(at + 4, end),
                        lint_kind: LintKind::Capitalization,
                        suggestions: vec![Suggestion::ReplaceWith(fixed.chars().collect())],
                        message: "Nach »bei« ist das Wort ein Nomen und wird großgeschrieben."
                            .to_owned(),
                        priority: 31,
                    });
                }
            }
            // *spricht Fließend Englisch*: the adverb in front of a language is not a noun.
            if c == 'F'
                && at >= 2
                && source[at - 1] == ' '
                && source[at - 2].is_lowercase()
                && source[at..].starts_with(&['F', 'l', 'i', 'e', 'ß', 'e', 'n', 'd', ' '][..])
            {
                let (end, language) = word_at(at + 9);
                if language.chars().next().is_some_and(char::is_uppercase)
                    && LANGUAGES.contains(&language.to_lowercase().as_str())
                {
                    lints.push(Lint {
                        span: Span::new(at, at + 8),
                        lint_kind: LintKind::Capitalization,
                        suggestions: vec![Suggestion::ReplaceWith("fließend".chars().collect())],
                        message: "»fließend« ist hier ein Adverb und wird kleingeschrieben."
                            .to_owned(),
                        priority: 31,
                    });
                }
                let _ = end;
            }
            // *Weißt du, wie spät ist es?*: the indirect question puts the verb last.
            if c == ','
                && source[at..].starts_with(
                    &[
                        ',', ' ', 'w', 'i', 'e', ' ', 's', 'p', 'ä', 't', ' ', 'i', 's', 't', ' ',
                        'e', 's',
                    ][..],
                )
                && source.get(at + 17).is_none_or(|c| !is_letter(*c))
            {
                lints.push(Lint {
                    span: Span::new(at + 11, at + 17),
                    lint_kind: LintKind::Grammar,
                    suggestions: vec![Suggestion::ReplaceWith("es ist".chars().collect())],
                    message: "Im indirekten Fragesatz steht das Verb am Ende: »wie spät es ist«."
                        .to_owned(),
                    priority: 31,
                });
            }
            // *Wir haben und gestern getroffen*: *uns* typed as *und*.
            if c == 'u'
                && at >= 6
                && source[at..].starts_with(&['u', 'n', 'd', ' '][..])
                && source[at - 1] == ' '
            {
                let before: String = source[..at - 1].iter().collect();
                let mut words = before.split_whitespace().rev();
                let verb = words.next().unwrap_or("");
                let subject = words.next().unwrap_or("");
                let (end, next) = word_at(at + 4);
                let _ = end;
                if subject.eq_ignore_ascii_case("wir")
                    && matches!(
                        verb,
                        "haben"
                            | "sind"
                            | "müssen"
                            | "sollen"
                            | "können"
                            | "wollen"
                            | "treffen"
                            | "sehen"
                            | "besuchen"
                            | "hatten"
                            | "waren"
                    )
                    && matches!(
                        next.as_str(),
                        "gestern"
                            | "heute"
                            | "morgen"
                            | "bald"
                            | "jetzt"
                            | "später"
                            | "oft"
                            | "schon"
                            | "noch"
                            | "nicht"
                            | "endlich"
                            | "wieder"
                    )
                {
                    lints.push(Lint {
                        span: Span::new(at, at + 3),
                        lint_kind: LintKind::Spelling,
                        suggestions: vec![Suggestion::ReplaceWith("uns".chars().collect())],
                        message: "Hier ist das Pronomen »uns« gemeint, nicht »und«.".to_owned(),
                        priority: 31,
                    });
                }
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
            "Bei fragen melden Sie sich.",
            "Das war schön.\n\nWir haben und gestern im Park getroffen.",
            "Wir haben und gestern im Park getroffen.",
            "Weißt du, wie spät ist es?",
            "Sie spricht Fließend Englisch.",
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
            "Bei Fragen melden Sie sich.",
            "Wir haben uns gestern im Park getroffen.",
            "Das war schön.\n\nWir haben uns gestern im Park getroffen.",
            "Wir haben Brot und gestern Kuchen gekauft.",
            "Wie spät ist es?",
            "Weißt du, wie spät es ist?",
            "Sie spricht fließend Englisch.",
            "Fließend Wasser ist wichtig.",
            "Das Haus hat 3 Zimmer Wohnung.",
            "Es gibt 18 Jahre.",
            "Das ist ein 5-Kilo Paket.",
        ] {
            let expected = usize::from(text.contains("5-Kilo"));
            assert_eq!(count(text), expected, "{text}");
        }
    }
}
