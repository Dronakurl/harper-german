//! *Mein Bruder und ich geht ins Kino*, *Gestern war Peter und Maria hier*: two subjects joined
//! by *und* take the verb in the plural.

use std::sync::Arc;

use harper_core::{
    Document, Token, TokenKind, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
    spell::Dictionary,
};

use crate::grammar::determiners::determiner_readings;
use crate::grammar::noun_phrase::lowercase_of;
use crate::grammar::token_text::starts_uppercase;
use crate::morphology::{MorphologyExt, NumberSet, PersonSet};
use crate::spell::compact_dict::CompactDictionary;
use crate::spell::curated_german_dictionary;

/// The singular forms of the auxiliaries and *sein*, with their plural.
const IRREGULAR: &[(&str, &str)] = &[
    ("ist", "sind"),
    ("war", "waren"),
    ("wird", "werden"),
    ("wurde", "wurden"),
    ("hat", "haben"),
    ("hatte", "hatten"),
    ("kann", "können"),
    ("konnte", "konnten"),
    ("muss", "müssen"),
    ("musste", "mussten"),
    ("will", "wollen"),
    ("soll", "sollen"),
    ("darf", "dürfen"),
    ("geht", "gehen"),
    ("kommt", "kommen"),
];

/// Reports a verb in the singular next to two subjects joined by *und*.
///
/// Each subject is a personal pronoun, a name, or a determiner with a noun (and adjectives
/// between). The pair either opens the sentence with the verb behind it, or follows the verb
/// in second place (*Gestern war Peter und Maria hier*). The verb has to be a form that is
/// singular only: the auxiliaries and *sein* from a table, or a form in *-t* that the
/// dictionary records for the third person singular and not the plural, whose plural is the
/// infinitive.
pub struct GermanCoordinatedSubject {
    dictionary: Arc<CompactDictionary>,
}

impl Default for GermanCoordinatedSubject {
    fn default() -> Self {
        Self {
            dictionary: curated_german_dictionary(),
        }
    }
}

impl GermanCoordinatedSubject {
    /// The end (exclusive) of a subject starting at `at`: a pronoun, a name, or a determiner,
    /// adjectives and a noun.
    fn subject_end(&self, words: &[&Token], at: usize, document: &Document) -> Option<usize> {
        let token = words.get(at)?;
        if !token.kind.is_word() {
            return None;
        }
        let word = lowercase_of(token, document);
        if ["ich", "du", "er", "sie", "wir", "ihr"].contains(&word.as_str()) {
            return Some(at + 1);
        }
        if determiner_readings(&word).is_some() {
            let mut head = at + 1;
            while words
                .get(head)
                .is_some_and(|t| t.kind.is_adjective() && !starts_uppercase(t, document))
            {
                head += 1;
            }
            let noun = words.get(head)?;
            return (noun.kind.is_word() && starts_uppercase(noun, document)).then_some(head + 1);
        }
        // A name: a capitalized word that is no determiner, preposition or conjunction.
        (starts_uppercase(token, document)
            && (token.kind.is_proper_noun() || token.kind.is_noun())
            && !token.kind.is_preposition()
            && !token.kind.is_conjunction())
        .then_some(at + 1)
    }

    /// The plural of a finite verb that is singular only.
    fn plural_of(&self, verb: &Token, document: &Document) -> Option<String> {
        let word = lowercase_of(verb, document);
        if let Some((_, plural)) = IRREGULAR.iter().find(|(singular, _)| *singular == word) {
            return Some((*plural).to_owned());
        }
        let stem = word.strip_suffix('t')?;
        let metadata = self
            .dictionary
            .get_word_metadata(&word.chars().collect::<Vec<_>>())?;
        let agreement = metadata.verb_agreement();
        if !agreement.person.contains(PersonSet::THIRD)
            || !agreement.number.contains(NumberSet::SINGULAR)
        {
            return None;
        }
        let stem = stem.strip_suffix('e').unwrap_or(stem);
        let infinitive = format!("{stem}en");
        self.dictionary
            .get_word_metadata_exact(&infinitive.chars().collect::<Vec<_>>())
            .is_some_and(|m| m.is_verb())
            .then_some(infinitive)
    }

    fn report(&self, verb: &Token, document: &Document) -> Option<Lint> {
        let plural = self.plural_of(verb, document)?;
        Some(Lint {
            span: verb.span,
            lint_kind: LintKind::Agreement,
            suggestions: vec![Suggestion::replace_with_match_case(
                plural.chars().collect(),
                document.get_span_content(&verb.span),
            )],
            message: format!(
                "Zwei mit »und« verbundene Subjekte verlangen den Plural: »{plural}«."
            ),
            priority: 31,
        })
    }
}

impl Linter for GermanCoordinatedSubject {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            let is_und = |at: usize| {
                words
                    .get(at)
                    .is_some_and(|t| lowercase_of(t, document) == "und")
            };
            // *sowohl … als auch*, *weder … noch*, *zwischen … und*: other constructions.
            if words.iter().any(|t| {
                matches!(
                    lowercase_of(t, document).as_str(),
                    "sowohl" | "weder" | "zwischen" | "sowie" | "beziehungsweise" | "bzw"
                )
            }) {
                continue;
            }

            // *Mein Bruder und ich geht …*
            // Two names may be one place (*Wallis und Futuna*), so one of the two has to be a
            // pronoun or a phrase with a determiner.
            let is_name = |start: usize, end: usize| {
                end == start + 1 && starts_uppercase(words[start], document)
            };
            if let Some(first_end) = self.subject_end(&words, 0, document)
                && is_und(first_end)
                && let Some(second_end) = self.subject_end(&words, first_end + 1, document)
                && !(is_name(0, first_end) && is_name(first_end + 1, second_end))
                && let Some(verb) = words.get(second_end)
                && verb.kind.is_word()
                && !starts_uppercase(verb, document)
                && let Some(lint) = self.report(verb, document)
            {
                lints.push(lint);
                continue;
            }

            // *Gestern war Peter und Maria hier*: an adverb or a phrase in front, the verb, then
            // the pair, which must end the clause or stand before an adverb or a participle.
            for verb_at in 1..words.len().min(4) {
                let verb = words[verb_at];
                if !verb.kind.is_word() || starts_uppercase(verb, document) {
                    continue;
                }
                let Some(first_end) = self.subject_end(&words, verb_at + 1, document) else {
                    continue;
                };
                if !is_und(first_end) {
                    continue;
                }
                let Some(second_end) = self.subject_end(&words, first_end + 1, document) else {
                    continue;
                };
                let closes = words.get(second_end).is_none_or(|t| {
                    matches!(t.kind, TokenKind::Punctuation(_))
                        || (!starts_uppercase(t, document)
                            && (t.kind.is_adverb() || t.kind.is_verb())
                            && !t.kind.is_preposition())
                });
                // Only the front field before the verb: one word or a short phrase without
                // a verb of its own.
                // Only an adverb of time or place in front: *Gestern war …*. A noun phrase there
                // is the subject itself (*Die Massage war ein Traum und …*).
                let front_is_plain = verb_at == 1
                    && matches!(
                        lowercase_of(words[0], document).as_str(),
                        "gestern"
                            | "heute"
                            | "morgen"
                            | "damals"
                            | "dann"
                            | "jetzt"
                            | "nun"
                            | "dort"
                            | "hier"
                            | "da"
                            | "bald"
                            | "später"
                            | "früher"
                            | "oft"
                            | "immer"
                            | "auch"
                            | "plötzlich"
                            | "endlich"
                            | "zuerst"
                    );
                if closes
                    && front_is_plain
                    && let Some(lint) = self.report(verb, document)
                {
                    lints.push(lint);
                    break;
                }
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Verlangt den Plural beim Verb, wenn zwei Subjekte mit »und« verbunden sind."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanCoordinatedSubject;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn fixes(text: &str) -> Vec<String> {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanCoordinatedSubject::default()
            .lint(&document)
            .into_iter()
            .flat_map(|lint| lint.suggestions)
            .map(|suggestion| suggestion.to_string())
            .collect()
    }

    #[test]
    fn a_singular_verb_with_two_subjects_is_reported() {
        for (text, fixed) in [
            ("Mein Bruder und ich geht ins Kino.", "gehen"),
            (
                "Die Geschäftsstelle und das Archiv wurde verlegt.",
                "wurden",
            ),
            ("Gestern war Peter und Maria hier.", "waren"),
            ("Der Hund und die Katze spielt im Garten.", "spielen"),
        ] {
            assert_eq!(fixes(text), [format!("Replace with: “{fixed}”")], "{text}");
        }
    }

    #[test]
    fn plural_verbs_and_other_constructions_are_quiet() {
        for text in [
            "Mein Bruder und ich gehen ins Kino.",
            "Peter und Maria kommen morgen.",
            "Sowohl Peter als auch Maria kommt.",
            "Gestern war Peter krank und Maria hier.",
            "Er sagt, Peter und Maria kommen.",
            "Das Haus und der Garten sind groß.",
            "Heute kauft Peter Brot und Butter.",
        ] {
            assert!(fixes(text).is_empty(), "{text}: {:?}", fixes(text));
        }
    }
}
