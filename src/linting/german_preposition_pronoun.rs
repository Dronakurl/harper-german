//! A personal pronoun in the wrong case behind a preposition: *mit **ihn***,
//! *für **ihm***, *ohne **mir***, *bei **dich***.

use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Token, TokenKind, TokenStringExt};

use crate::grammar::noun_phrase::lowercase_of;

/// Prepositions that take the dative only.
const DATIVE: &[&str] = &[
    "mit",
    "bei",
    "von",
    "zu",
    "aus",
    "seit",
    "gegenüber",
    "nebst",
    "samt",
];

/// Prepositions that take the accusative only. *um* and *ohne* also open an
/// infinitive group (*um mir zu helfen*, *ohne mir zu sagen*), where the
/// pronoun is the infinitive's; they count only without a *zu* behind them.
const ACCUSATIVE: &[&str] = &["für", "gegen", "durch", "um", "ohne", "wider"];

/// The pronouns whose case shows, with the form of the other case. *uns*,
/// *euch*, *sie*, *es* are the same in both and never reported.
const DATIVE_FORMS: &[(&str, &str)] = &[("mir", "mich"), ("dir", "dich"), ("ihm", "ihn")];

#[derive(Default)]
pub struct GermanPrepositionPronoun;

impl Linter for GermanPrepositionPronoun {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|t| !t.kind.is_whitespace())
                .collect();
            for at in 0..words.len().saturating_sub(1) {
                let preposition = lowercase_of(words[at], document);
                let pronoun_token = words[at + 1];
                let pronoun = document.get_span_content_str(&pronoun_token.span);
                let wants_dative = DATIVE.contains(&preposition.as_str());
                let wants_accusative = ACCUSATIVE.contains(&preposition.as_str());
                if !wants_dative && !wants_accusative {
                    continue;
                }
                // A separable particle at the end of its clause is no
                // preposition: *Er nimmt ihn mit*. Behind it starts a new
                // phrase, and the particle comes after the pronoun, not before.
                // Before it, the preposition has to open its phrase.
                if at > 0
                    && matches!(words[at - 1].kind, TokenKind::Word(_))
                    && DATIVE_FORMS.iter().any(|(d, a)| {
                        lowercase_of(words[at - 1], document) == *d
                            || lowercase_of(words[at - 1], document) == *a
                    })
                {
                    continue;
                }
                // *um mir zu helfen*, *ohne ihn zu fragen*, *um seiner willen*.
                let clause_rest: Vec<String> = words[at + 2..]
                    .iter()
                    .take_while(|t| !matches!(t.kind, TokenKind::Punctuation(_)))
                    .map(|t| lowercase_of(t, document))
                    .collect();
                if matches!(preposition.as_str(), "um" | "ohne")
                    && clause_rest
                        .iter()
                        .any(|w| w == "zu" || w == "willen" || w.contains("zu"))
                {
                    continue;
                }
                let found = if wants_dative {
                    DATIVE_FORMS
                        .iter()
                        .find(|(_, accusative)| pronoun == *accusative)
                        .map(|(dative, accusative)| (*accusative, *dative, "den Dativ"))
                } else {
                    DATIVE_FORMS
                        .iter()
                        .find(|(dative, _)| pronoun == *dative)
                        .map(|(dative, accusative)| (*dative, *accusative, "den Akkusativ"))
                };
                let Some((written, fixed, case)) = found else {
                    continue;
                };
                lints.push(Lint {
                    span: pronoun_token.span,
                    lint_kind: LintKind::Grammar,
                    suggestions: vec![Suggestion::ReplaceWith(fixed.chars().collect())],
                    priority: 31,
                    message: format!(
                        "»{preposition}« verlangt {case}: »{preposition} {fixed}«, nicht »{written}«."
                    ),
                });
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Prüft den Fall eines Pronomens nach einer Präposition: »mit ihm«, »für ihn«, »ohne mich«."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanPrepositionPronoun;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn reported(text: &str) -> Vec<String> {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanPrepositionPronoun
            .lint(&document)
            .iter()
            .map(|l| document.get_span_content_str(&l.span))
            .collect()
    }

    #[test]
    fn a_pronoun_in_the_wrong_case_is_reported() {
        assert_eq!(reported("Er geht mit ihn ins Kino."), ["ihn"]);
        assert_eq!(reported("Das ist ein Geschenk für ihm."), ["ihm"]);
        assert_eq!(reported("Ohne mir geht das nicht."), ["mir"]);
        assert_eq!(reported("Ich wohne bei dich um die Ecke."), ["dich"]);
    }

    #[test]
    fn the_right_case_and_infinitive_groups_are_quiet() {
        for text in [
            "Er geht mit ihm ins Kino.",
            "Das ist für ihn.",
            "Er kam, um mir zu helfen.",
            "Er ging, ohne mir etwas zu sagen.",
            "Ich wohne bei euch.",
            "Er nimmt ihn mit zu seiner Mutter.",
            "Kommst du mit uns?",
            "Das ist für sie.",
        ] {
            assert!(reported(text).is_empty(), "{text}: {:?}", reported(text));
        }
    }
}
