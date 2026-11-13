//! *Ich habe zwei Bruder*, *vor zwei Woche*, *in fünf Minute*: a number above one counts a
//! plural.

use harper_core::{
    Document, Token, TokenKind, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
    spell::Dictionary,
};

use crate::grammar::noun_gender::NounGender;
use crate::grammar::prepositions::preposition_government;
use crate::grammar::token_text::{lowercase_of, starts_uppercase};
use crate::morphology::{Case, CaseSet, MorphologyExt};
use crate::spell::lexical_classes::NUMERALS;

/// Nouns of measure that stay singular behind a number: *zwei Mal*, *drei Euro*, *zehn Grad*,
/// *zwei Glas Wasser*, *zehn Mann*.
const MEASURES: &[&str] = &[
    "Mal",
    "Euro",
    "Cent",
    "Dollar",
    "Franken",
    "Pfund",
    "Mark",
    "Prozent",
    "Grad",
    "Meter",
    "Kilometer",
    "Zentimeter",
    "Millimeter",
    "Quadratmeter",
    "Kubikmeter",
    "Liter",
    "Milliliter",
    "Kilo",
    "Kilogramm",
    "Gramm",
    "Hektar",
    "Stück",
    "Glas",
    "Paar",
    "Dutzend",
    "Zoll",
    "Fuß",
    "Uhr",
    "Mann",
    "Sack",
    "Fass",
    "Kasten",
    "Blatt",
    "Satz",
    "Byte",
    "Bit",
    "Volt",
    "Watt",
    "Ampere",
    "Grad",
    "Punkt",
    "Schuss",
    "Takt",
    "Knoten",
    "Bogen",
    "Seemeile",
    "Elle",
];

/// Reports a noun in the singular behind a cardinal number above one, with the plural the
/// dictionary has for it: *zwei Bruder* → *Brüder*, *drei Kinds* → *Kinder*, and in the dative
/// behind a preposition with its *-n*: *vor zwei Woche* → *Wochen*.
///
/// Only a noun the dictionary knows as surely singular is read (see
/// [`NounGender::gender_of`]), so *zwei Lehrer* and *drei Mädchen* stay; nouns of measure stay
/// singular (*zwei Mal*, *drei Euro*); and the phrase has to end at the noun, so *zwei Bruder
/// Klaus* and *drei Kinds-Köpfe* are left alone.
pub struct GermanNumeralPlural {
    nouns: NounGender,
}

impl Default for GermanNumeralPlural {
    fn default() -> Self {
        Self::new()
    }
}

impl GermanNumeralPlural {
    pub fn new() -> Self {
        Self {
            nouns: NounGender::new(),
        }
    }

    /// Is the token a number above one: a numeral word, or digits from 2 to 999 (from 1000 on
    /// mostly a year)?
    fn counts_more_than_one(token: &Token, document: &Document) -> bool {
        match token.kind {
            TokenKind::Number(_) => document
                .get_span_content_str(&token.span)
                .parse::<u32>()
                .is_ok_and(|n| (2..1000).contains(&n)),
            TokenKind::Word(_) => {
                let word = lowercase_of(token, document);
                !starts_uppercase(token, document)
                    && NUMERALS.contains(&word)
                    && !word.starts_with("ein")
                    && word != "null"
            }
            _ => false,
        }
    }

    /// The plural of `singular` the dictionary has: a noun that is not itself surely singular.
    fn plural_of(&self, singular: &str) -> Option<String> {
        let umlauted = umlaut_last_vowel(singular);
        let mut candidates = Vec::new();
        if let Some(umlauted) = &umlauted {
            candidates.push(format!("{umlauted}e"));
            candidates.push(format!("{umlauted}er"));
            candidates.push(umlauted.clone());
        }
        for ending in ["e", "en", "n", "er", "nen", "s"] {
            candidates.push(format!("{singular}{ending}"));
        }
        candidates.into_iter().find(|candidate| {
            self.nouns
                .dictionary()
                .get_word_metadata(&candidate.chars().collect::<Vec<_>>())
                .is_some_and(|metadata| metadata.is_noun())
                && self
                    .nouns
                    .gender_of(candidate)
                    .is_none_or(|(_, surely_singular)| !surely_singular)
                && self.nouns.is_surely_plural(candidate)
        })
    }
}

/// The word with the last umlautable vowel of its stem umlauted (*Bruder* → *Brüder*, *Haus* →
/// *Häus*), or `None`.
fn umlaut_last_vowel(word: &str) -> Option<String> {
    let mut chars: Vec<char> = word.chars().collect();
    let at = chars
        .iter()
        .rposition(|c| matches!(c, 'a' | 'o' | 'u' | 'A' | 'O' | 'U'))?;
    let at = if at > 0 && chars[at] == 'u' && matches!(chars[at - 1], 'a' | 'A') {
        at - 1
    } else {
        at
    };
    chars[at] = match chars[at] {
        'a' => 'ä',
        'o' => 'ö',
        'u' => 'ü',
        'A' => 'Ä',
        'O' => 'Ö',
        'U' => 'Ü',
        other => other,
    };
    Some(chars.into_iter().collect())
}

impl Linter for GermanNumeralPlural {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        let content = document.get_full_content();

        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();

            for at in 0..words.len() {
                if !Self::counts_more_than_one(words[at], document) {
                    continue;
                }
                // *um zwei*, *Artikel 20 Grundgesetz*, *1969 / 70 Präsident*, *0-05 Strich*: a
                // number behind a noun or a sign is a time, a label or part of a range.
                if at > 0 {
                    let previous = words[at - 1];
                    let previous_word = lowercase_of(previous, document);
                    if ["um", "nr"].contains(&previous_word.as_str())
                        || (matches!(previous.kind, TokenKind::Punctuation(_))
                            && content.get(previous.span.start) != Some(&','))
                        || (previous.kind.is_word()
                            && starts_uppercase(previous, document)
                            && !(at == 1
                                && (previous.kind.is_preposition()
                                    || previous.kind.is_determiner())))
                    {
                        continue;
                    }
                }
                // *2jährigen*: the number is glued to the word behind it.
                if content
                    .get(words[at].span.end)
                    .is_some_and(|c| !c.is_whitespace())
                {
                    continue;
                }
                // Adjectives may stand between the number and the noun: *zwei kleine Bruder*.
                let mut head = at + 1;
                while words.get(head).is_some_and(|token| {
                    token.kind.is_adjective()
                        && !starts_uppercase(token, document)
                        && !token.kind.is_determiner()
                }) {
                    head += 1;
                }
                let Some(noun) = words.get(head) else {
                    continue;
                };
                if !matches!(noun.kind, TokenKind::Word(_)) || !starts_uppercase(noun, document) {
                    continue;
                }
                // The phrase ends at the noun: not *zwei Bruder Klaus*, not *drei Kinds-Köpfe*.
                if words
                    .get(head + 1)
                    .is_some_and(|next| starts_uppercase(next, document) || next.kind.is_hyphen())
                    || content.get(noun.span.end) == Some(&'-')
                    || content.get(noun.span.end) == Some(&'.')
                        && content
                            .get(noun.span.end + 1)
                            .is_some_and(|c| c.is_alphanumeric())
                {
                    continue;
                }
                let text = document.get_span_content_str(&noun.span);
                if MEASURES.contains(&text.as_str()) {
                    continue;
                }
                // *drei Kinds*: the genitive singular.
                let singular = match self.nouns.gender_of(&text) {
                    Some((_, true)) => text.clone(),
                    _ => match text.strip_suffix('s') {
                        Some(stem)
                            if self.nouns.gender_of(stem).is_some_and(|(_, sure)| sure)
                                && !self
                                    .nouns
                                    .dictionary()
                                    .get_word_metadata(&text.chars().collect::<Vec<_>>())
                                    .is_some_and(|m| {
                                        m.noun_agreement()
                                            .number
                                            .contains(crate::morphology::NumberSet::PLURAL)
                                    }) =>
                        {
                            stem.to_owned()
                        }
                        _ => continue,
                    },
                };
                if MEASURES.contains(&singular.as_str()) {
                    continue;
                }
                let Some(mut plural) = self.plural_of(&singular) else {
                    continue;
                };
                // *vor zwei Wochen*, *seit drei Jahren*: behind a preposition that takes the
                // dative and nothing else here, the plural takes its *-n*.
                let dative = at
                    .checked_sub(1)
                    .and_then(|previous| {
                        preposition_government(&lowercase_of(words[previous], document))
                    })
                    .is_some_and(|preposition| {
                        preposition.cases == CaseSet::from(Case::Dative)
                            || ["vor", "in"]
                                .contains(&lowercase_of(words[at - 1], document).as_str())
                    });
                if dative && !plural.ends_with(['n', 's']) {
                    plural.push('n');
                }
                if plural == text {
                    continue;
                }
                lints.push(Lint {
                    span: noun.span,
                    lint_kind: LintKind::Agreement,
                    suggestions: vec![Suggestion::ReplaceWith(plural.chars().collect())],
                    message: format!(
                        "Nach einer Zahl über eins steht das Nomen im Plural: »{plural}«."
                    ),
                    priority: 31,
                });
            }
        }

        lints
    }

    fn description(&self) -> &str {
        "Setzt ein Nomen hinter einer Zahl über eins in den Plural (»zwei Brüder«, »vor zwei Wochen«)."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanNumeralPlural;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn fixes(text: &str) -> Vec<String> {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanNumeralPlural::new()
            .lint(&document)
            .into_iter()
            .flat_map(|lint| lint.suggestions)
            .map(|suggestion| suggestion.to_string())
            .collect()
    }

    #[test]
    fn a_singular_behind_a_number_gets_its_plural() {
        for (text, fixed) in [
            ("Ich habe zwei Bruder.", "Brüder"),
            ("Er hat drei Kinds.", "Kinder"),
            ("Vor zwei Woche war ich krank.", "Wochen"),
            ("Ich komme in fünf Minute.", "Minuten"),
            ("Sie hat drei Katze.", "Katzen"),
            ("Wir haben zwei Hund.", "Hunde"),
            ("Er wartete 20 Minute.", "Minuten"),
        ] {
            assert_eq!(fixes(text), [format!("Replace with: “{fixed}”")], "{text}");
        }
    }

    #[test]
    fn plurals_measures_and_names_are_quiet() {
        for text in [
            "Ich habe zwei Brüder.",
            "Das kostet drei Euro.",
            "Es ist zehn Grad warm.",
            "Ich war zwei Mal dort.",
            "Wir treffen uns um drei Uhr.",
            "Er hat zwei Lehrer.",
            "Sie hat drei Mädchen.",
            "Wir trinken zwei Glas Wasser.",
            "Im Jahr 1990 Wahl gewonnen.",
            "Das steht auf Seite 5 Absatz 2.",
            "Vor zwei Wochen war ich krank.",
            "Seit drei Jahren wohnen wir hier.",
        ] {
            assert!(fixes(text).is_empty(), "{text}: {:?}", fixes(text));
        }
    }
}
