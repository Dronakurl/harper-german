//! *Ich habe ein Brief geschrieben*, *Gibt es hier ein Supermarkt?*: a masculine object in the
//! accusative takes *einen*.

use harper_core::{
    Document, Span, Token, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
};

use crate::grammar::noun_gender::NounGender;
use crate::grammar::noun_phrase::lowercase_of;
use crate::grammar::token_text::starts_uppercase;
use crate::morphology::GenderSet;

const HABEN: &[&str] = &["habe", "hast", "hat", "haben", "habt", "hatte", "hatten"];

/// The nominative of the articles whose masculine accusative adds *-en*.
const ARTICLES: &[(&str, &str)] = &[
    ("ein", "einen"),
    ("kein", "keinen"),
    ("mein", "meinen"),
    ("dein", "deinen"),
    ("sein", "seinen"),
    ("unser", "unseren"),
    ("euer", "euren"),
];

/// Reports *ein*, *kein*, *mein* … in front of a noun that is only masculine, when the noun is
/// the object of
///
/// * a perfect with *haben*, right in front of its participle: *habe gestern ein Brief
///   geschrieben*;
/// * *es gibt*: *Gibt es hier ein Supermarkt?*.
///
/// Everything else is left to the nominative: *Das ist ein Brief*, *Ein Brief kam*.
#[derive(Default)]
pub struct GermanMasculineObject {
    nouns: NounGender,
}

impl GermanMasculineObject {
    /// The genders of the noun, or of the longest known word it ends on when the noun is a
    /// compound the dictionary does not list: *Supermarkt* → *Markt*.
    fn genders(&self, noun: &str) -> GenderSet {
        let known = self.nouns.genders(noun);
        if !known.is_empty() {
            return known;
        }
        let chars: Vec<char> = noun.chars().collect();
        for from in 1..chars.len().saturating_sub(3) {
            let mut tail: Vec<char> = chars[from..].to_vec();
            if let Some(first) = tail.first_mut() {
                *first = first.to_uppercase().next().unwrap_or(*first);
            }
            let genders = self.nouns.genders(&tail.iter().collect::<String>());
            if !genders.is_empty() {
                return genders;
            }
        }
        GenderSet::empty()
    }
}

impl Linter for GermanMasculineObject {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            let lower: Vec<String> = words.iter().map(|t| lowercase_of(t, document)).collect();
            let has_haben = lower.iter().any(|w| HABEN.contains(&w.as_str()));
            let has_gibt = lower
                .windows(2)
                .any(|w| (w[0] == "gibt" && w[1] == "es") || (w[0] == "es" && w[1] == "gibt"));
            if !has_haben && !has_gibt {
                continue;
            }
            for at in 1..words.len().saturating_sub(1) {
                let Some((_, accusative)) = ARTICLES.iter().find(|(n, _)| *n == lower[at]) else {
                    continue;
                };
                // A preposition or a genitive article in front: not an object.
                if (words[at - 1].kind.is_preposition()
                    && !starts_uppercase(words[at - 1], document))
                    || lower[at - 1] == "ist"
                {
                    continue;
                }
                let noun = words[at + 1];
                if !starts_uppercase(noun, document) || !noun.kind.is_word() {
                    continue;
                }
                let text = document.get_span_content_str(&noun.span);
                if self.genders(&text) != GenderSet::MASCULINE {
                    continue;
                }
                let after = words.get(at + 2).map(|t| lowercase_of(t, document));
                let participle = after.as_deref().is_some_and(|w| {
                    w.chars().count() >= 6
                        && w.starts_with("ge")
                        && (w.ends_with('t') || w.ends_with("en"))
                }) && words[at + 2].kind.is_verb();
                // The auxiliary stands in the same clause: no comma between them.
                let same_clause = lower[..at]
                    .iter()
                    .rposition(|w| HABEN.contains(&w.as_str()))
                    .is_some_and(|h| !words[h..at].iter().any(|t| t.kind.is_comma()));
                let ends_question =
                    has_gibt && words.get(at + 2).is_none_or(|t| t.kind.is_punctuation());
                if !(has_haben && participle && same_clause || ends_question) {
                    continue;
                }
                lints.push(Lint {
                    span: Span::new(words[at].span.start, words[at].span.end),
                    lint_kind: LintKind::Agreement,
                    suggestions: vec![Suggestion::ReplaceWith(accusative.chars().collect())],
                    message: format!(
                        "»{text}« ist hier das Objekt im Akkusativ: »{accusative} {text}«."
                    ),
                    priority: 31,
                });
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Setzt »einen« vor ein männliches Objekt im Akkusativ (»ein Brief geschrieben«)."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanMasculineObject;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn count(text: &str) -> usize {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanMasculineObject::default().lint(&document).len()
    }

    #[test]
    fn a_masculine_object_takes_einen() {
        for text in [
            "Ich habe gestern ein Brief geschrieben.",
            "Er hat sein Hund gesehen.",
            "Gibt es hier in der Nähe ein Supermarkt?",
            "Es gibt kein Bahnhof.",
        ] {
            assert_eq!(count(text), 1, "{text}");
        }
    }

    #[test]
    fn other_cases_are_quiet() {
        for text in [
            "Ich habe gestern einen Brief geschrieben.",
            "Das ist ein Brief.",
            "Ein Brief ist gekommen.",
            "Ich habe ein Buch gelesen.",
            "Er hat ein Haus gekauft.",
            "Gibt es hier ein Hotel?",
            "Ich habe mit ein Brief gespielt.",
            "Es gibt ein Problem.",
            "Ich habe ein Kind gesehen.",
            "Ein Hunger hat alle ergriffen, ein Rausch geht durch die Welt.",
        ] {
            assert_eq!(count(text), 0, "{text}");
        }
    }
}
