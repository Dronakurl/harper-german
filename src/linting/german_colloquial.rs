//! Colloquial short forms of chat and email: *is*, *nich*, *nen*, *ham*,
//! *kannste*. In formal text they are wrong, and the spell checker's guesses
//! for them are other words (*is* → *iss*, *nen* → *nenn*). This rule gives
//! the standard form instead, under a name of its own, so that it can be
//! switched off for informal writing.

use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Token, TokenStringExt};

use crate::linting::german_foreign_stretch;

/// The short form and its standard spelling.
pub(crate) const COLLOQUIAL: &[(&str, &str)] = &[
    ("is", "ist"),
    ("nich", "nicht"),
    ("nen", "einen"),
    ("nem", "einem"),
    ("ner", "einer"),
    ("ham", "haben"),
    ("nix", "nichts"),
    ("kannste", "kannst du"),
    ("haste", "hast du"),
    ("biste", "bist du"),
    ("willste", "willst du"),
    ("weißte", "weißt du"),
];

/// Is `word` one of the short forms? The spell checker leaves them to this rule.
pub(crate) fn is_colloquial(word: &str) -> bool {
    let lower = word.to_lowercase();
    COLLOQUIAL.iter().any(|(short, _)| *short == lower)
}

#[derive(Default)]
pub struct GermanColloquial;

impl Linter for GermanColloquial {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let tokens: Vec<&Token> = sentence
                .iter()
                .filter(|t| !t.kind.is_whitespace())
                .collect();
            for (index, word) in tokens.iter().enumerate() {
                let written = document.get_span_content_str(&word.span);
                // *is* in an English quotation is English.
                // Capitalized inside the sentence it is a name (*West Ham*); glued to
                // more letters, a piece of one (*Isère*).
                let capitalized = written.chars().next().is_some_and(char::is_uppercase);
                let glued = document
                    .get_full_content()
                    .get(word.span.end)
                    .is_some_and(|c| c.is_alphabetic());
                if !word.kind.is_word()
                    || !is_colloquial(&written)
                    || (capitalized && index > 0)
                    || glued
                    || german_foreign_stretch::in_foreign_stretch(&tokens, index, document, |t| {
                        t.kind.is_determiner()
                    })
                {
                    continue;
                }
                let lower = written.to_lowercase();
                let Some((_, standard)) = COLLOQUIAL.iter().find(|(short, _)| *short == lower)
                else {
                    continue;
                };
                let mut fixed: Vec<char> = standard.chars().collect();
                if written.chars().next().is_some_and(char::is_uppercase)
                    && let Some(first) = fixed.first_mut()
                {
                    *first = first.to_uppercase().next().unwrap_or(*first);
                }
                lints.push(Lint {
                    span: word.span,
                    lint_kind: LintKind::Style,
                    suggestions: vec![Suggestion::ReplaceWith(fixed.clone())],
                    priority: 25,
                    message: format!(
                        "»{written}« ist umgangssprachlich. In der Standardsprache: »{}«.",
                        fixed.iter().collect::<String>()
                    ),
                });
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Ersetzt umgangssprachliche Kurzformen durch die Standardform: »is« → »ist«, »nen« → »einen«."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanColloquial;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn fixes(text: &str) -> Vec<String> {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanColloquial
            .lint(&document)
            .into_iter()
            .flat_map(|l| l.suggestions)
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn short_forms_get_the_standard_one() {
        assert_eq!(fixes("Das is mir egal.").len(), 1);
        assert!(fixes("Hast du nen Stift?")[0].contains("einen"));
        assert!(fixes("Kannste mir helfen?")[0].contains("Kannst du"));
        assert!(fixes("Er musste gehen.").is_empty());
        assert!(fixes("Das ist mir egal.").is_empty());
    }
}
