//! *Bitte beachten sie die Hinweise*: the polite *Sie* written small.

use harper_core::{
    Document, Token, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
};

use crate::grammar::noun_phrase::lowercase_of;
use crate::grammar::token_text::starts_lowercase;
use crate::linting::german_relative_clause_comma::GermanRelativeClauseComma;
use crate::spell::curated_german_dictionary;
use harper_core::spell::Dictionary;

/// Reports *sie* behind a verb in *-en* in a clause with *bitte*: *Bitte beachten sie …*, *Bei
/// Fragen wenden sie sich bitte an …*, *Rufen sie mich bitte an*. A request with *bitte* is
/// addressed to someone, so *sie* there is the polite *Sie*, not *they*.
#[derive(Default)]
pub struct GermanPoliteSie;

impl Linter for GermanPoliteSie {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            // The clause: up to the first comma.
            let end = words
                .iter()
                .position(|t| t.kind.is_comma())
                .unwrap_or(words.len());
            let clause = &words[..end];
            let has_bitte = clause.iter().any(|t| lowercase_of(t, document) == "bitte");
            if !has_bitte {
                continue;
            }
            for at in 1..clause.len() {
                let token = clause[at];
                if document.get_span_content_str(&token.span) != "sie" {
                    continue;
                }
                let verb = clause[at - 1];
                let verb_word = lowercase_of(verb, document);
                // The verb in *-en* right in front, in first or second place: *Bitte beachten
                // sie*, *Rufen sie*, *Bei Fragen wenden sie*.
                // The verb opens the request (*Rufen sie*), follows *bitte* (*Bitte beachten
                // sie*) or a fronted phrase with a preposition (*Bei Fragen wenden sie*); a
                // subject in front (*Die Kinder haben sie*) makes *sie* an object.
                let first_or_second = match at - 1 {
                    0 => true,
                    1 => lowercase_of(clause[0], document) == "bitte",
                    2 => clause[0].kind.is_preposition(),
                    _ => false,
                };
                if !first_or_second
                    || !verb_word.ends_with("en")
                    || !(starts_lowercase(verb, document) || at == 1)
                    || !(GermanRelativeClauseComma::is_verb_token(verb, document)
                        || curated_german_dictionary()
                            .get_word_metadata(&verb_word.chars().collect::<Vec<_>>())
                            .is_some_and(|metadata| metadata.is_verb()))
                {
                    continue;
                }
                lints.push(Lint {
                    span: token.span,
                    lint_kind: LintKind::Capitalization,
                    suggestions: vec![Suggestion::ReplaceWith(vec!['S', 'i', 'e'])],
                    message: "Die höfliche Anrede »Sie« wird großgeschrieben.".to_owned(),
                    priority: 31,
                });
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Schreibt die höfliche Anrede »Sie« in einer Bitte groß."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanPoliteSie;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn count(text: &str) -> usize {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanPoliteSie.lint(&document).len()
    }

    #[test]
    fn a_small_sie_in_a_request_is_reported() {
        for text in [
            "Bitte beachten sie die Hinweise auf der Rückseite.",
            "Bei Fragen wenden sie sich bitte an unseren Kundenservice.",
            "Bitte antworten sie bis Freitag.",
            "Rufen sie mich bitte zurück.",
        ] {
            assert_eq!(count(text), 1, "{text}");
        }
        for text in [
            "Bitte beachten Sie die Hinweise.",
            "Die Kinder haben sie bitte gefragt.",
            "Sie wollen, dass wir sie bitten.",
            "Bitte sag ihr, dass sie kommen soll.",
            "Die Eltern kommen, wenn sie können.",
        ] {
            assert_eq!(count(text), 0, "{text}");
        }
    }
}
