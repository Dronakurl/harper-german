//! *Sie hat Ihre Schlüssel verloren*: a capital *Ihre* in a sentence about *sie* (she).

use harper_core::{
    Document, Punctuation, Token, TokenKind, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
};

use crate::grammar::noun_phrase::lowercase_of;
use crate::grammar::token_text::starts_uppercase;

const THIRD_PERSON_SINGULAR: &[&str] = &[
    "hat", "ist", "war", "hatte", "wird", "wurde", "kann", "konnte", "muss", "musste", "will",
    "wollte", "soll", "sollte", "darf", "mag", "macht", "geht", "kommt", "sagt", "liebt", "sieht",
];

const POSSESSIVES: &[&str] = &["Ihr", "Ihre", "Ihren", "Ihrem", "Ihrer", "Ihres"];

/// Reports *Ihr*, *Ihre* … in mid-sentence when the sentence starts with *Sie* and a singular verb
/// (*Sie hat*, *Sie ist*): the subject is *sie* (she), and the possessive is hers, in lower case.
/// Nothing that addresses a reader (*Ihnen*, *bitte*, a second *Sie*, a question) is touched.
#[derive(Default)]
pub struct GermanCapitalIhr;

impl Linter for GermanCapitalIhr {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            if words.len() < 4
                || document.get_span_content_str(&words[0].span) != "Sie"
                || !THIRD_PERSON_SINGULAR.contains(&lowercase_of(words[1], document).as_str())
            {
                continue;
            }
            let addresses_reader = words.iter().skip(1).any(|t| {
                let word = lowercase_of(t, document);
                (starts_uppercase(t, document) && matches!(word.as_str(), "sie" | "ihnen"))
                    || matches!(word.as_str(), "bitte" | "danke")
                    || matches!(t.kind, TokenKind::Punctuation(Punctuation::Question))
            });
            if addresses_reader {
                continue;
            }
            for token in words.iter().skip(2) {
                let written = document.get_span_content_str(&token.span);
                if POSSESSIVES.contains(&written.as_str()) {
                    let lower = written.to_lowercase();
                    lints.push(Lint {
                        span: token.span,
                        lint_kind: LintKind::Capitalization,
                        suggestions: vec![Suggestion::ReplaceWith(lower.chars().collect())],
                        message:
                            "Hier ist von »sie« (einer Frau) die Rede: »ihre« schreibt man klein."
                                .to_owned(),
                        priority: 31,
                    });
                }
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Schreibt »ihr« klein, wenn der Satz mit »Sie hat« oder »Sie ist« von einer Frau handelt."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanCapitalIhr;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn count(text: &str) -> usize {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanCapitalIhr.lint(&document).len()
    }

    #[test]
    fn a_capital_possessive_for_she_is_reported() {
        for text in [
            "Sie hat Ihre Schlüssel verloren.",
            "Sie ist mit Ihrem Mann in den Urlaub gefahren.",
            "Sie wollte Ihr Auto verkaufen.",
        ] {
            assert_eq!(count(text), 1, "{text}");
        }
    }

    #[test]
    fn the_polite_address_is_quiet() {
        for text in [
            "Sie hat ihre Schlüssel verloren.",
            "Haben Sie Ihre Schlüssel verloren?",
            "Sie können Ihre Schlüssel hier abholen.",
            "Sie haben Ihre Schlüssel verloren.",
            "Sie hat Ihnen Ihre Schlüssel gegeben.",
            "Sie hat bitte Ihre Schlüssel dabei.",
            "Ihre Schlüssel liegen hier.",
        ] {
            assert_eq!(count(text), 0, "{text}");
        }
    }
}
