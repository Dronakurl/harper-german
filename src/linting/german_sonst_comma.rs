//! *Wir müssen uns beeilen sonst verpassen wir den Zug*, *Kannst du mir das Salz geben bitte?*:
//! a comma in front of *sonst* that opens a main clause, and in front of a closing *bitte*.

use harper_core::{
    Document, Punctuation, Token, TokenKind, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
};

use crate::grammar::noun_phrase::lowercase_of;
use crate::grammar::token_text::starts_uppercase;
use crate::linting::german_relative_clause_comma::GermanRelativeClauseComma;

const SUBJECTS: &[&str] = &[
    "ich", "du", "er", "sie", "es", "wir", "ihr", "man", "der", "die", "das", "dann",
];

/// Requires a comma
///
/// * in front of *sonst* when a verb and its subject follow: *beeilen sonst verpassen wir*;
/// * in front of *bitte* at the end of a question or request that ends on a verb:
///   *geben bitte?* → *geben, bitte?*.
#[derive(Default)]
pub struct GermanSonstComma;

impl GermanSonstComma {
    fn lint_for(at: usize, words: &[&Token], message: &str) -> Lint {
        Lint {
            span: words[at - 1].span,
            lint_kind: LintKind::Punctuation,
            suggestions: vec![Suggestion::InsertAfter(vec![','])],
            message: message.to_owned(),
            priority: 31,
        }
    }
}

impl Linter for GermanSonstComma {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            for at in 2..words.len() {
                if !matches!(words[at].kind, TokenKind::Word(_)) {
                    continue;
                }
                let previous = words[at - 1];
                if !matches!(previous.kind, TokenKind::Word(_)) {
                    continue;
                }
                let word = lowercase_of(words[at], document);
                if starts_uppercase(words[at], document) {
                    continue;
                }
                if word == "sonst" {
                    let verb = words.get(at + 1);
                    let subject = words.get(at + 2);
                    if let (Some(verb), Some(subject)) = (verb, subject)
                        && GermanRelativeClauseComma::is_verb_token(verb, document)
                        && SUBJECTS.contains(&lowercase_of(subject, document).as_str())
                    {
                        lints.push(Self::lint_for(
                            at,
                            &words,
                            "Vor »sonst« steht ein Komma, wenn ein Hauptsatz folgt.",
                        ));
                    }
                } else if word == "bitte"
                    && words.get(at + 1).is_some_and(|next| {
                        matches!(
                            next.kind,
                            TokenKind::Punctuation(Punctuation::Question | Punctuation::Period)
                        )
                    })
                    && words.get(at + 2).is_none()
                    && GermanRelativeClauseComma::is_verb_token(previous, document)
                    && !starts_uppercase(previous, document)
                {
                    lints.push(Self::lint_for(
                        at,
                        &words,
                        "Das nachgestellte »bitte« wird mit einem Komma abgetrennt.",
                    ));
                }
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Setzt das Komma vor »sonst« mit Hauptsatz und vor einem nachgestellten »bitte«."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanSonstComma;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn count(text: &str) -> usize {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanSonstComma.lint(&document).len()
    }

    #[test]
    fn sonst_with_a_clause_and_a_closing_bitte_need_a_comma() {
        for text in [
            "Wir müssen uns beeilen sonst verpassen wir den Zug.",
            "Beeil dich sonst kommst du zu spät.",
            "Kannst du mir das Salz geben bitte?",
            "Wir sollten jetzt gehen bitte.",
        ] {
            assert_eq!(count(text), 1, "{text}");
        }
    }

    #[test]
    fn with_a_comma_or_a_different_use_the_text_is_quiet() {
        for text in [
            "Wir müssen uns beeilen, sonst verpassen wir den Zug.",
            "Kannst du mir das Salz geben, bitte?",
            "Wir können gehen, sonst nichts.",
            "Hast du sonst noch Fragen an mich?",
            "Was kann ich sonst tun?",
            "Ja bitte.",
            "Wie bitte?",
            "Bitte gib mir das Salz.",
            "Hilf mir bitte.",
        ] {
            assert_eq!(count(text), 0, "{text}");
        }
    }
}
