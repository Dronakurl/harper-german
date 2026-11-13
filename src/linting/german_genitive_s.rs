//! A masculine or neuter noun after *des* without its genitive ending: *die Farbe des Auto*,
//! *der Preis des Haus*.

use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::spell::Dictionary;
use harper_core::{Document, Punctuation, Token, TokenKind, TokenStringExt};

use crate::grammar::noun_gender::NounGender;
use crate::grammar::token_text::{lowercase_of, starts_lowercase};
use crate::morphology::Gender;

/// Determiners that are only ever a masculine or neuter genitive singular.
const GENITIVE_DETERMINERS: &[&str] = &[
    "des", "eines", "meines", "deines", "seines", "unseres", "eures", "keines",
];

/// Nouns whose genitive may go without the ending: months (*des Januar*) and a few loans
/// (*des Islam*, *des Barock*).
const ENDING_OPTIONAL: &[&str] = &[
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
    "Islam",
    "Barock",
    "Rokoko",
    "Biedermeier",
];

/// Reports the noun behind *des* (or *eines*, *meines* …) that has lost its genitive *-s*.
///
/// Only a noun the base dictionary records with one gender, masculine or neuter, and whose
/// genitive with *-s* or *-es* it lists, is taken. Nouns in *-e* and *-n* are left out: most of
/// them are weak (*des Kollegen*, *des Herrn*), which is another rule's business.
#[derive(Default)]
pub struct GermanGenitiveS {
    nouns: NounGender,
}

impl GermanGenitiveS {
    pub fn new() -> Self {
        Self::default()
    }

    fn genitive_of(&self, noun: &str) -> Option<String> {
        if noun.ends_with(['e', 'n'])
            || noun.chars().all(|c| !c.is_lowercase())
            || ENDING_OPTIONAL.contains(&noun)
        {
            return None;
        }
        // *eines Bronzestift*: a compound the dictionary does not list takes the genitive of its
        // last element, a noun of four letters or more.
        if self.nouns.gender_of(noun).is_none()
            && self
                .nouns
                .dictionary()
                .get_word_metadata(&noun.chars().collect::<Vec<_>>())
                .is_none()
        {
            let chars: Vec<char> = noun.chars().collect();
            return (1..chars.len().saturating_sub(3)).find_map(|at| {
                let mut tail: Vec<char> = chars[at..].to_vec();
                tail[0] = tail[0].to_uppercase().next()?;
                let tail: String = tail.into_iter().collect();
                let genitive = self.genitive_of_listed(&tail)?;
                let ending = genitive.strip_prefix(tail.as_str())?;
                Some(format!("{noun}{ending}"))
            });
        }
        self.genitive_of_listed(noun)
    }

    /// [`Self::genitive_of`] for a noun the dictionary lists.
    fn genitive_of_listed(&self, noun: &str) -> Option<String> {
        if noun.ends_with(['e', 'n']) {
            return None;
        }
        // The number does not matter: behind *des* the noun is a singular, and a plural spelled
        // the same (*Zimmer*) has no genitive in *-s*.
        let (gender, _) = self.nouns.gender_of(noun)?;
        if gender == Gender::Feminine {
            return None;
        }
        // After a sibilant only *-es* is a genitive: *des Hauses*, not *des Kaukasuss*.
        let endings: &[&str] = if noun.ends_with(['s', 'ß', 'x', 'z']) {
            &["es"]
        } else {
            &["s", "es"]
        };
        endings
            .iter()
            .map(|ending| format!("{noun}{ending}"))
            .find(|form| {
                let chars: Vec<char> = form.chars().collect();
                self.nouns
                    .dictionary()
                    .get_word_metadata(&chars)
                    .is_some_and(|metadata| metadata.is_noun())
            })
    }
}

impl GermanGenitiveS {
    /// Is the word after the noun one that starts something new: a verb (*des Auto gefällt*),
    /// another determiner (*des Budgetdefizit des Staates*) or a punctuation mark? A compound
    /// (*des Auto-*), a coordination (*des Sturm und Drang*) and a foreign title (*des
    /// Department of*, *des Film noir*) leave the noun alone.
    fn ends_the_phrase(next: Option<&&Token>, document: &Document) -> bool {
        let Some(next) = next else {
            return true;
        };
        match &next.kind {
            TokenKind::Punctuation(p) => matches!(
                p,
                Punctuation::Period
                    | Punctuation::Comma
                    | Punctuation::Semicolon
                    | Punctuation::Colon
                    | Punctuation::Bang
                    | Punctuation::Question
            ),
            TokenKind::Word(_) => {
                let lower = lowercase_of(next, document);
                lower != "und"
                    && lower != "oder"
                    && starts_lowercase(next, document)
                    && (next.kind.is_determiner() || (next.kind.is_verb() && !next.kind.is_noun()))
            }
            _ => false,
        }
    }
}

impl Linter for GermanGenitiveS {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|t| !t.kind.is_whitespace())
                .collect();
            for (at, determiner) in words.iter().enumerate() {
                let determiner_text = lowercase_of(determiner, document);
                if !GENITIVE_DETERMINERS.contains(&determiner_text.as_str()) {
                    continue;
                }
                // Directly behind the determiner: with an adjective between, a name keeps its bare
                // form (*des heiligen Ludwig*, *des östlichen Frankreich*).
                let Some(noun) = words
                    .get(at + 1)
                    .filter(|t| matches!(t.kind, TokenKind::Word(_)))
                else {
                    continue;
                };
                let noun_text = document.get_span_content_str(&noun.span);
                if !noun_text.chars().next().is_some_and(char::is_uppercase)
                    || noun_text.chars().count() < 4
                    || noun_text.chars().skip(1).any(char::is_uppercase)
                {
                    continue;
                }
                if !Self::ends_the_phrase(words.get(at + 2), document) {
                    continue;
                }
                let Some(genitive) = self.genitive_of(&noun_text) else {
                    continue;
                };
                lints.push(Lint {
                    span: noun.span,
                    lint_kind: LintKind::Agreement,
                    suggestions: vec![Suggestion::ReplaceWith(genitive.chars().collect())],
                    message: format!(
                        "Nach »{determiner_text}« steht das Nomen im Genitiv: »{genitive}«."
                    ),
                    priority: 31,
                });
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Ergänzt das Genitiv-s nach »des«: »die Farbe des Autos«, »der Preis des Hauses«."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanGenitiveS;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::{Linter, Suggestion};

    fn fixes(text: &str) -> Vec<String> {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanGenitiveS::new()
            .lint(&document)
            .into_iter()
            .map(|lint| match &lint.suggestions[0] {
                Suggestion::ReplaceWith(chars) => chars.iter().collect(),
                other => panic!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn the_genitive_ending_is_added() {
        for (text, fix) in [
            ("Die Farbe des Auto gefällt mir.", "Autos"),
            ("Der Preis des Haus ist hoch.", "Hauses"),
            ("Das Ende des Film war traurig.", "Films"),
            ("Die Tür des Zimmer ist offen.", "Zimmers"),
            (
                "Er war Leiter des Organisationsaufbau beim Verband.",
                "Organisationsaufbaus",
            ),
        ] {
            assert_eq!(fixes(text), [fix], "{text}");
        }
    }

    #[test]
    fn correct_and_unclear_genitives_are_quiet() {
        for text in [
            "Die Farbe des Autos gefällt mir.",
            "Die Frau des Herrn ist da.",
            "Das Büro des Kollegen ist leer.",
            "Im Laufe des Januar wird es kalt.",
            "Die Krönung des Kaiser Wilhelm war prunkvoll.",
            "Der Bau des Auto-Werks dauerte lange.",
            "Er trug den Orden des heiligen Ludwig.",
            "Er war Leiter des Department of Archives.",
            "Das ist ein Werk des Sturm und Drang.",
            "Die Farbe der Autos gefällt mir.",
            "Er hat die Geologie des Kaukasus untersucht.",
        ] {
            assert!(fixes(text).is_empty(), "{text}");
        }
    }
}
