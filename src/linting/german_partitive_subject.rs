//! *Keiner von uns haben Zeit*, *Die meisten von uns hat Zeit*, *Die Anzahl der Teilnehmer
//! sind gestiegen*: the verb agrees with the head of the subject, not with what follows it.

use std::sync::Arc;

use harper_core::{
    Document, Token, TokenKind, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
    spell::Dictionary,
};

use crate::grammar::noun_gender::NounGender;
use crate::grammar::noun_phrase::lowercase_of;
use crate::grammar::token_text::starts_uppercase;
use crate::morphology::{MorphologyExt, NumberSet, PersonSet};
use crate::spell::compact_dict::CompactDictionary;
use crate::spell::curated_german_dictionary;

const SINGULAR_PLURAL: &[(&str, &str)] = &[
    ("ist", "sind"),
    ("war", "waren"),
    ("hat", "haben"),
    ("hatte", "hatten"),
    ("wird", "werden"),
    ("wurde", "wurden"),
    ("kann", "können"),
    ("muss", "müssen"),
    ("will", "wollen"),
    ("soll", "sollen"),
    ("darf", "dürfen"),
    ("gibt", "geben"),
    ("nimmt", "nehmen"),
    ("weiß", "wissen"),
];

/// Pronouns that are singular: *keiner von uns*, *jeder der Teilnehmer*.
const SINGULAR_HEADS: &[&str] = &[
    "keiner", "keine", "jeder", "jede", "einer", "eine", "niemand",
];

/// Quantifiers that are plural: *die meisten von uns*, *viele von euch*.
const PLURAL_HEADS: &[&str] = &[
    "meisten", "viele", "alle", "einige", "beide", "wenige", "manche",
];

/// Nouns of quantity whose plural attribute may carry the verb (*Ein Teil der Schüler fehlten*
/// is accepted), so they are left alone.
const COLLECTIVES: &[&str] = &[
    "Teil",
    "Menge",
    "Reihe",
    "Mehrheit",
    "Hälfte",
    "Großteil",
    "Vielzahl",
    "Gruppe",
    "Paar",
    "Anteil",
    "Minderheit",
    "Drittel",
    "Viertel",
    "Prozent",
    "Handvoll",
    "Dutzend",
];

/// Reports the finite verb behind a subject whose head is singular (or plural) while the words
/// it governs are plural (or singular):
///
/// * *Keiner von uns haben* → *hat*, *Jeder der Teilnehmer bekommen* → *bekommt*;
/// * *Die meisten von uns hat* → *haben*;
/// * *Die Anzahl der Teilnehmer sind* → *ist*, *Das Ergebnis der Wahlen sind* → *ist*: a
///   singular noun with a genitive attribute in the plural; not a noun of quantity
///   (*Ein Teil der Schüler fehlten*).
///
/// Only at the start of the sentence, with the verb right behind the subject.
pub struct GermanPartitiveSubject {
    dictionary: Arc<CompactDictionary>,
    nouns: NounGender,
}

impl Default for GermanPartitiveSubject {
    fn default() -> Self {
        Self {
            dictionary: curated_german_dictionary(),
            nouns: NounGender::new(),
        }
    }
}

impl GermanPartitiveSubject {
    fn agreement(&self, word: &str) -> Option<(PersonSet, NumberSet)> {
        let metadata = self
            .dictionary
            .get_word_metadata(&word.chars().collect::<Vec<_>>())?;
        let agreement = metadata.verb_agreement();
        Some((agreement.person, agreement.number))
    }

    /// The singular of a plural finite verb: *haben* → *hat*, *bekommen* → *bekommt*.
    fn singular_of(&self, verb: &str) -> Option<String> {
        if let Some((singular, _)) = SINGULAR_PLURAL.iter().find(|(_, plural)| *plural == verb) {
            return Some((*singular).to_owned());
        }
        let stem = verb.strip_suffix("en")?;
        let candidate = if stem.ends_with(['d', 't']) {
            format!("{stem}et")
        } else {
            format!("{stem}t")
        };
        let (person, number) = self.agreement(&candidate)?;
        (person.contains(PersonSet::THIRD) && number.contains(NumberSet::SINGULAR))
            .then_some(candidate)
    }

    /// The plural of a singular finite verb: *hat* → *haben*, *kommt* → *kommen*.
    fn plural_of(&self, verb: &str) -> Option<String> {
        if let Some((_, plural)) = SINGULAR_PLURAL
            .iter()
            .find(|(singular, _)| *singular == verb)
        {
            return Some((*plural).to_owned());
        }
        let stem = verb.strip_suffix('t')?;
        let stem = stem.strip_suffix('e').unwrap_or(stem);
        let candidate = format!("{stem}en");
        self.dictionary
            .get_word_metadata_exact(&candidate.chars().collect::<Vec<_>>())
            .is_some_and(|m| m.is_verb())
            .then_some(candidate)
    }

    /// The index behind a plural attribute that starts at `at`: *von uns*, *der Teilnehmer*,
    /// *der Wahlen*.
    fn plural_attribute_end(
        &self,
        words: &[&Token],
        at: usize,
        document: &Document,
        any_noun: bool,
    ) -> Option<usize> {
        let first = lowercase_of(words.get(at)?, document);
        let second = words.get(at + 1)?;
        if first == "von"
            && matches!(
                lowercase_of(second, document).as_str(),
                "uns" | "euch" | "ihnen"
            )
        {
            return Some(at + 2);
        }
        if first == "der" || first == "von" && lowercase_of(second, document) == "den" {
            let mut head = if first == "der" { at + 1 } else { at + 2 };
            while words
                .get(head)
                .is_some_and(|t| t.kind.is_adjective() && !starts_uppercase(t, document))
            {
                head += 1;
            }
            let noun = words.get(head)?;
            let text = document.get_span_content_str(&noun.span);
            return (starts_uppercase(noun, document)
                && (any_noun || self.nouns.is_surely_plural(&text)))
            .then_some(head + 1);
        }
        None
    }

    fn report(verb: &Token, fixed: String, message: &str) -> Lint {
        Lint {
            span: verb.span,
            lint_kind: LintKind::Agreement,
            suggestions: vec![Suggestion::ReplaceWith(fixed.chars().collect())],
            message: message.to_owned(),
            priority: 31,
        }
    }
}

impl Linter for GermanPartitiveSubject {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            if words.len() < 4 {
                continue;
            }
            let first = lowercase_of(words[0], document);

            // *Keiner von uns haben*, *Jeder der Teilnehmer bekommen*.
            if SINGULAR_HEADS.contains(&first.as_str())
                && let Some(end) = self.plural_attribute_end(&words, 1, document, true)
                && let Some(verb) = words.get(end)
                && matches!(verb.kind, TokenKind::Word(_))
                && !starts_uppercase(verb, document)
                && let Some(fixed) = self.singular_of(&lowercase_of(verb, document))
            {
                lints.push(Self::report(
                    verb,
                    fixed,
                    "Das Subjekt ist »keiner«, »jeder« oder »einer«: Das Verb steht im Singular.",
                ));
                continue;
            }

            // *Die meisten von uns hat*.
            let (head_at, head) = if first == "die" {
                (1, lowercase_of(words[1], document))
            } else {
                (0, first.clone())
            };
            if PLURAL_HEADS.contains(&head.as_str())
                && let Some(end) = self.plural_attribute_end(&words, head_at + 1, document, true)
                && let Some(verb) = words.get(end)
                && matches!(verb.kind, TokenKind::Word(_))
                && !starts_uppercase(verb, document)
                && (SINGULAR_PLURAL
                    .iter()
                    .any(|(singular, _)| *singular == lowercase_of(verb, document))
                    || self.agreement(&lowercase_of(verb, document)).is_some_and(
                        |(person, number)| {
                            person.contains(PersonSet::THIRD) && number == NumberSet::SINGULAR
                        },
                    ))
                && let Some(fixed) = self.plural_of(&lowercase_of(verb, document))
            {
                lints.push(Self::report(
                    verb,
                    fixed,
                    "Das Subjekt ist ein Plural (»die meisten«, »viele«): Das Verb steht im Plural.",
                ));
                continue;
            }

            // *Die Anzahl der Teilnehmer sind*: article, singular noun, plural genitive.
            if !matches!(
                first.as_str(),
                "die" | "das" | "der" | "ein" | "eine" | "mein" | "meine"
            ) {
                continue;
            }
            let noun = words[1];
            let text = document.get_span_content_str(&noun.span);
            if !starts_uppercase(noun, document)
                || COLLECTIVES.contains(&text.as_str())
                || !self.nouns.gender_of(&text).is_some_and(|(_, sure)| sure)
            {
                continue;
            }
            if let Some(end) = self.plural_attribute_end(&words, 2, document, true)
                && let Some(verb) = words.get(end)
                && matches!(verb.kind, TokenKind::Word(_))
                && !starts_uppercase(verb, document)
                && let Some(fixed) = self.singular_of(&lowercase_of(verb, document))
            {
                lints.push(Self::report(
                    verb,
                    fixed,
                    &format!("Das Subjekt ist »{text}«, ein Singular: Das Verb steht im Singular."),
                ));
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Prüft die Zahl des Verbs bei »keiner von uns«, »die meisten von uns« und »die Anzahl der …«."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanPartitiveSubject;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn fixes(text: &str) -> Vec<String> {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanPartitiveSubject::default()
            .lint(&document)
            .into_iter()
            .flat_map(|lint| lint.suggestions)
            .map(|suggestion| suggestion.to_string())
            .collect()
    }

    #[test]
    fn the_verb_agrees_with_the_head() {
        for (text, fixed) in [
            ("Keiner von uns haben Zeit.", "hat"),
            ("Jeder der Teilnehmer bekommen ein Geschenk.", "bekommt"),
            ("Die meisten von uns hat Zeit.", "haben"),
            ("Die Anzahl der Teilnehmer sind gestiegen.", "ist"),
            ("Das Ergebnis der Wahlen sind eindeutig.", "ist"),
        ] {
            assert_eq!(fixes(text), [format!("Replace with: “{fixed}”")], "{text}");
        }
    }

    #[test]
    fn agreeing_verbs_and_collectives_are_quiet() {
        for text in [
            "Keiner von uns hat Zeit.",
            "Die meisten von uns haben Zeit.",
            "Die Anzahl der Teilnehmer ist gestiegen.",
            "Ein Teil der Schüler fehlen heute.",
            "Die Eltern der Kinder sind da.",
            "Viele von uns kommen.",
        ] {
            assert!(fixes(text).is_empty(), "{text}: {:?}", fixes(text));
        }
    }
}
