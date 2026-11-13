//! *Er brauch das nicht*: the bare stem behind a subject in the third person singular.

use std::sync::Arc;

use harper_core::{
    Document, Token, TokenKind, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
    spell::Dictionary,
};

use crate::grammar::token_text::lowercase_of;
use crate::morphology::{MorphologyExt, NumberSet, PersonSet};
use crate::spell::compact_dict::CompactDictionary;
use crate::spell::curated_german_dictionary;

/// Stems whose third person is irregular, so that stem + *-t* would be another form: *hab* →
/// *hat*, not *habt*.
const IRREGULAR: &[&str] = &[
    "hab", "werd", "wiss", "könn", "müss", "woll", "soll", "dürf", "mög", "sei", "tu", "weiß",
    "kann", "muss", "will", "darf", "mag", "wird", "ist",
];

/// Reports the bare stem of a verb behind *er*, *es* or *man*, with the third person singular:
/// *Er brauch das nicht* → *braucht*, *Es klapp nicht* → *klappt*.
///
/// The stem has to be a verb form the dictionary knows (the colloquial first person or the
/// imperative), and stem + *-t* a form it records for the third person singular. A strong verb
/// changes its vowel (*er nimmt*), so a suggestion is made only where stem + *-t* is that form;
/// otherwise nothing is reported.
pub struct GermanBareStem {
    dictionary: Arc<CompactDictionary>,
}

impl Default for GermanBareStem {
    fn default() -> Self {
        Self {
            dictionary: curated_german_dictionary(),
        }
    }
}

impl GermanBareStem {
    fn third_person(&self, stem: &str) -> Option<String> {
        // A stem in *-t* is mostly a form of its own (*hat*, *tut*, *wart*).
        if IRREGULAR.contains(&stem) || stem.chars().count() < 3 || stem.ends_with('t') {
            return None;
        }
        // The stem is no word of its own in small letters, or a verb form that is not the third
        // person already (*hat*); and its infinitive is a verb.
        let stem_chars: Vec<char> = stem.chars().collect();
        if let Some(own) = self.dictionary.get_word_metadata_exact(&stem_chars) {
            let agreement = own.verb_agreement();
            // *gelang* is a preterite, *schon* an adverb.
            let preterite = own
                .verb
                .as_ref()
                .and_then(|verb| verb.verb_forms)
                .is_some_and(|forms| forms.contains(harper_core::VerbFormFlags::PAST));
            if !own.is_verb()
                || preterite
                || own.adverb.is_some()
                || own.preposition
                || own.conjunction.is_some()
                || (agreement.person.contains(PersonSet::THIRD)
                    && agreement.number.contains(NumberSet::SINGULAR))
            {
                return None;
            }
        }
        let infinitive: Vec<char> = format!("{stem}en").chars().collect();
        if !self
            .dictionary
            .get_word_metadata_exact(&infinitive)
            .is_some_and(|m| m.is_verb())
        {
            return None;
        }
        // *er findet*, *er arbeitet*: a stem in *-d* or *-t* takes *-et*.
        let candidate = if stem.ends_with(['d', 't']) {
            format!("{stem}et")
        } else {
            format!("{stem}t")
        };
        let metadata = self
            .dictionary
            .get_word_metadata(&candidate.chars().collect::<Vec<_>>())?;
        let agreement = metadata.verb_agreement();
        (agreement.person.contains(PersonSet::THIRD)
            && agreement.number.contains(NumberSet::SINGULAR))
        .then_some(candidate)
    }
}

impl Linter for GermanBareStem {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            for at in 1..words.len() {
                let subject = lowercase_of(words[at - 1], document);
                if !["er", "es", "man"].contains(&subject.as_str()) {
                    continue;
                }
                let verb = words[at];
                if !matches!(verb.kind, TokenKind::Word(_)) {
                    continue;
                }
                let written = document.get_span_content_str(&verb.span);
                if !written.chars().all(char::is_lowercase) {
                    continue;
                }
                // *Hab ich, er komm…*: only in front of a word or the end, not before an
                // apostrophe (*er komm's*).
                if document.get_full_content().get(verb.span.end) == Some(&'\'') {
                    continue;
                }
                let Some(fixed) = self.third_person(&written) else {
                    continue;
                };
                lints.push(Lint {
                    span: verb.span,
                    lint_kind: LintKind::Agreement,
                    suggestions: vec![Suggestion::ReplaceWith(fixed.chars().collect())],
                    message: format!(
                        "Nach »{subject}« steht die dritte Person: »{fixed}«, nicht »{written}«."
                    ),
                    priority: 31,
                });
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Setzt den bloßen Verbstamm nach »er«, »es« oder »man« in die dritte Person (»er braucht«)."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanBareStem;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn fixes(text: &str) -> Vec<String> {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanBareStem::default()
            .lint(&document)
            .into_iter()
            .flat_map(|lint| lint.suggestions)
            .map(|suggestion| suggestion.to_string())
            .collect()
    }

    #[test]
    fn the_bare_stem_behind_er_gets_its_t() {
        assert_eq!(fixes("Er brauch das nicht."), ["Replace with: “braucht”"]);
        assert_eq!(fixes("Er mach das morgen."), ["Replace with: “macht”"]);
        assert_eq!(
            fixes("Das weiß man, wenn man frag."),
            ["Replace with: “fragt”"]
        );
    }

    #[test]
    fn right_forms_are_quiet() {
        for text in [
            "Er braucht das nicht.",
            "Er hat das nicht.",
            "Er kann das.",
            "Wie er sagt, kommt er.",
            "Er nimmt den Bus.",
            "Er komm's schon.",
            "Er selbst kam nicht.",
            "Er weiß es.",
            "Es gelang ihm nicht.",
            "Heute war er schon da.",
            "Er bald.",
        ] {
            assert!(fixes(text).is_empty(), "{text}: {:?}", fixes(text));
        }
    }
}
