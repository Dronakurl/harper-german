//! *Ich habe keine Zeit nicht.*: a *nicht* after a phrase with *kein* at the end of the clause
//! negates it a second time.

use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Span, Token, TokenKind, TokenStringExt};

use crate::grammar::token_text::lowercase_of;

const KEIN: &[&str] = &["kein", "keine", "keinen", "keinem", "keiner", "keines"];

/// Reports *kein* + noun + *nicht* + end of the sentence. With anything between the noun and
/// *nicht*, or a comma (*Keine Zeit, nicht heute*), the *nicht* may belong to something else, so
/// only this tight pattern is taken.
#[derive(Default)]
pub struct GermanDoubleNegation;

impl Linter for GermanDoubleNegation {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|t| !t.kind.is_whitespace())
                .collect();
            for at in 2..words.len() {
                let [kein, noun, nicht] = [words[at - 2], words[at - 1], words[at]];
                let closes = words
                    .get(at + 1)
                    .is_none_or(|t| t.kind.is_sentence_terminator());
                if !closes
                    || lowercase_of(nicht, document) != "nicht"
                    || !noun.kind.is_noun()
                    || !KEIN.contains(&lowercase_of(kein, document).as_str())
                    || !matches!(noun.kind, TokenKind::Word(_))
                {
                    continue;
                }
                lints.push(Lint {
                    span: Span::new(noun.span.end, nicht.span.end),
                    lint_kind: LintKind::Grammar,
                    suggestions: vec![Suggestion::Remove],
                    message: "Doppelte Verneinung: »kein« verneint schon, »nicht« fällt weg."
                        .to_string(),
                    priority: 31,
                });
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Meldet ein zweites »nicht« nach »kein« + Nomen am Satzende: »Ich habe keine Zeit nicht.«"
    }
}

#[cfg(test)]
mod tests {
    use super::GermanDoubleNegation;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn fixed(text: &str) -> Vec<String> {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanDoubleNegation
            .lint(&document)
            .into_iter()
            .map(|lint| {
                let mut chars: Vec<char> = text.chars().collect();
                lint.suggestions[0].apply(lint.span, &mut chars);
                chars.into_iter().collect()
            })
            .collect()
    }

    #[test]
    fn the_second_negation_is_removed() {
        assert_eq!(
            fixed("Ich habe keine Zeit nicht."),
            ["Ich habe keine Zeit."]
        );
        assert_eq!(
            fixed("Er hat keinen Hunger nicht!"),
            ["Er hat keinen Hunger!"]
        );
    }

    #[test]
    fn other_negations_stay() {
        for text in [
            "Keine Zeit, nicht heute.",
            "Ich habe keine Zeit.",
            "Ich habe nicht nichts getan.",
            "Ich habe keine Zeit nicht nur heute.",
        ] {
            assert!(fixed(text).is_empty(), "{text}");
        }
    }
}
