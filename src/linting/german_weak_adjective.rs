//! *Er ist der bester Spieler*: behind the definite article an adjective takes the weak ending.

use std::sync::Arc;

use harper_core::{
    Document, Token, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
    spell::Dictionary,
};

use crate::grammar::determiners::determiner_readings;
use crate::grammar::noun_phrase::lowercase_of;
use crate::grammar::token_text::starts_uppercase;
use crate::spell::compact_dict::CompactDictionary;
use crate::spell::curated_german_dictionary;

/// The definite article and the words declined like it, after which an adjective only ends in
/// *-e* or *-en*.
const DEFINITE: &[&str] = &[
    "der", "die", "das", "dem", "den", "des", "dieser", "diese", "dieses", "diesem", "diesen",
    "jener", "jene", "jenes", "jenem", "jenen", "jeder", "jede", "jedes", "jedem", "jeden",
];

/// Adjectives whose bare form ends in *-er*.
const BARE_IN_ER: &[&str] = &[
    "bitter", "munter", "heiter", "finster", "lecker", "sauber", "sicher", "teuer", "sauer",
    "mager", "locker", "tapfer", "bieder", "wacker", "lauter", "düster", "clever", "super",
    "ander", "lockerer",
];

/// Reports an adjective with a strong ending (*-er*, *-es*, *-em*) between a definite article
/// and its noun: *der bester Spieler* → *beste*, *das neues Auto* → *neue*, *dem großem Haus* →
/// *großen*. The adjective's stem with *-e* has to be a word the dictionary knows, and the
/// adjective itself must not be a bare form that ends that way (*lecker*, *sauber*).
pub struct GermanWeakAdjective {
    dictionary: Arc<CompactDictionary>,
}

impl Default for GermanWeakAdjective {
    fn default() -> Self {
        Self {
            dictionary: curated_german_dictionary(),
        }
    }
}

impl GermanWeakAdjective {
    fn weak_forms(&self, article: &str, adjective: &str) -> Option<Vec<String>> {
        let stem = ["er", "es", "em"]
            .iter()
            .find_map(|ending| adjective.strip_suffix(ending))?;
        let known = |word: &str| {
            self.dictionary
                .get_word_metadata(&word.chars().collect::<Vec<_>>())
                .is_some_and(|metadata| metadata.is_adjective())
        };
        // *lecker*, *sauber*, *heiter*: the bare adjective itself ends in *-er*.
        if self
            .dictionary
            .get_word_metadata_exact(&adjective.chars().collect::<Vec<_>>())
            .is_some_and(|metadata| metadata.is_adjective() && !metadata.is_noun())
            && !known(&format!("{stem}e"))
        {
            return None;
        }
        let weak_e = format!("{stem}e");
        let weak_en = format!("{stem}en");
        if !known(&weak_e) {
            return None;
        }
        // The ending the article asks for: *der*, *die*, *das* in the nominative take *-e*,
        // the others *-en*.
        Some(match article {
            "der" | "die" | "das" | "dieser" | "diese" | "dieses" | "jener" | "jene" | "jenes"
            | "jeder" | "jede" | "jedes" => vec![weak_e, weak_en],
            _ => vec![weak_en],
        })
    }
}

impl Linter for GermanWeakAdjective {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            for at in 1..words.len().saturating_sub(1) {
                let article = lowercase_of(words[at - 1], document);
                if !DEFINITE.contains(&article.as_str()) || determiner_readings(&article).is_none()
                {
                    continue;
                }
                // A capital article in mid-sentence is part of a name: *Die Zeit*.
                if at > 1 && starts_uppercase(words[at - 1], document) {
                    continue;
                }
                let adjective = words[at];
                let noun = words[at + 1];
                if !adjective.kind.is_adjective()
                    || starts_uppercase(adjective, document)
                    || !noun.kind.is_word()
                    || !starts_uppercase(noun, document)
                {
                    continue;
                }
                let written = lowercase_of(adjective, document);
                // *…, der später …*, *…, die physiologischer Weise …*: behind a comma the word is a
                // relative pronoun; an adverb reading (*später*) or a bare adjective in *-er*
                // (*lecker*) is no declined adjective; *die moderner PCs* is a demonstrative with a
                // genitive plural.
                if (at > 1 && words[at - 2].kind.is_comma())
                    || BARE_IN_ER.contains(&written.as_str())
                    || (article == "die" && written.ends_with("er"))
                {
                    continue;
                }
                let Some(forms) = self.weak_forms(&article, &written) else {
                    continue;
                };
                lints.push(Lint {
                    span: adjective.span,
                    lint_kind: LintKind::Agreement,
                    suggestions: forms
                        .iter()
                        .map(|form| Suggestion::ReplaceWith(form.chars().collect()))
                        .collect(),
                    message: format!(
                        "Nach »{article}« endet das Adjektiv auf -e oder -en, nicht »{written}«."
                    ),
                    priority: 31,
                });
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Verlangt nach dem bestimmten Artikel die schwache Endung des Adjektivs (»der beste Spieler«)."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanWeakAdjective;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn flagged(text: &str) -> Vec<String> {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanWeakAdjective::default()
            .lint(&document)
            .iter()
            .map(|lint| document.get_span_content_str(&lint.span))
            .collect()
    }

    #[test]
    fn a_strong_ending_after_the_article_is_reported() {
        for (text, word) in [
            ("Er ist der bester Spieler.", "bester"),
            ("Das neues Auto ist rot.", "neues"),
            ("Wir wohnen in dem großem Haus.", "großem"),
            ("Dieser kleiner Hund bellt.", "kleiner"),
            ("Der früherer Polizeichef kam.", "früherer"),
        ] {
            assert_eq!(flagged(text), [word], "{text}");
        }
    }

    #[test]
    fn weak_endings_and_bare_adjectives_are_quiet() {
        for text in [
            "Er ist der beste Spieler.",
            "Das ist der saubere Tisch.",
            "Die Zeit Neuer Medien.",
            "Das lecker Essen.",
            "Ich gebe dem Lehrer das Buch.",
            "Er ist der jüngere Bruder.",
            "Das war Paul, der später Brückens Mann wurde.",
        ] {
            assert!(flagged(text).is_empty(), "{text}: {:?}", flagged(text));
        }
    }
}
