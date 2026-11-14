//! *Er hat viel Fehler gemacht*: in front of a plural, *viel* is *viele*.

use harper_core::{
    Document, Token, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
};

use crate::grammar::noun_gender::NounGender;
use crate::grammar::noun_phrase::lowercase_of;
use crate::grammar::token_text::starts_uppercase;
use crate::morphology::{GenderSet, MorphologyExt};

/// Words in front of *viel* that make it an adverb or a degree: *so viel*, *wie viel*, *zu viel*.
const DEGREE: &[&str] = &[
    "so", "wie", "zu", "sehr", "genauso", "ebenso", "gleich", "wie", "als", "nicht", "noch",
    "recht", "ziemlich", "bis", "um", "mit", "von", "für", "ohne",
];

/// Masculine nouns in *-er* that are not counted: *viel Zucker*.
const MASS_NOUNS: &[&str] = &[
    "Zucker",
    "Kummer",
    "Ärger",
    "Hunger",
    "Donner",
    "Dünger",
    "Humor",
    "Pfeffer",
    "Honig",
    "Sauerstoff",
    "Kaffeesatz",
    "Zimmer",
    "Fieber",
    "Wasser",
    "Leder",
    "Lärm",
    "Reichtum",
];

/// Reports *viel* in front of a noun that is surely a plural: *viel Fehler* → *viele Fehler*.
#[derive(Default)]
pub struct GermanVielPlural {
    nouns: NounGender,
}

impl GermanVielPlural {
    /// Is `word` in the dictionary as a noun in the plural and not in the singular?
    fn plural_only_noun(&self, word: &str) -> bool {
        use harper_core::spell::Dictionary;
        let chars: Vec<char> = word.chars().collect();
        self.nouns
            .dictionary()
            .get_word_metadata_exact(&chars)
            .filter(|m| m.is_noun())
            .is_some_and(|m| {
                let number = m.noun_agreement().number;
                number == crate::morphology::NumberSet::PLURAL
            })
    }
}

impl Linter for GermanVielPlural {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            for at in 0..words.len().saturating_sub(1) {
                // *viele Freunden*: a nominative or accusative quantifier with a dative plural.
                if matches!(
                    lowercase_of(words[at], document).as_str(),
                    "viele" | "alle" | "einige" | "mehrere" | "wenige" | "manche" | "beide"
                ) && let Some(noun) = words.get(at + 1)
                    && noun.kind.is_word()
                    && starts_uppercase(noun, document)
                {
                    let text = document.get_span_content_str(&noun.span);
                    let plural = text.strip_suffix('n').unwrap_or("");
                    if self.nouns.is_dative_plural(&text)
                        || (self.plural_only_noun(plural)
                            && plural.strip_suffix('e').is_some_and(|singular| {
                                let genders = self.nouns.genders(singular);
                                !genders.is_empty()
                                    && genders != crate::morphology::GenderSet::FEMININE
                            }))
                    {
                        let fixed = &text[..text.len() - 1];
                        lints.push(Lint {
                            span: noun.span,
                            lint_kind: LintKind::Agreement,
                            suggestions: vec![Suggestion::ReplaceWith(fixed.chars().collect())],
                            message: format!(
                                "Nach »{}« steht der Plural ohne -n: »{fixed}«.",
                                lowercase_of(words[at], document)
                            ),
                            priority: 31,
                        });
                    }
                    continue;
                }
                if lowercase_of(words[at], document) != "viel"
                    || (at == 0 && !starts_uppercase(words[at], document))
                    || (at > 0
                        && (starts_uppercase(words[at], document)
                            || DEGREE.contains(&lowercase_of(words[at - 1], document).as_str())))
                {
                    continue;
                }
                let noun = words[at + 1];
                if !noun.kind.is_word() || !starts_uppercase(noun, document) {
                    continue;
                }
                let text = document.get_span_content_str(&noun.span);
                // *Fehler*, *Lehrer*: the singular is spelled like the plural, so a masculine
                // noun in *-er* counts unless it is a mass noun (*viel Zucker*).
                let same_form_plural = text.ends_with("er")
                    && text.chars().count() >= 5
                    && self.nouns.genders(&text) == GenderSet::MASCULINE
                    && !MASS_NOUNS.contains(&text.as_str());
                if !(self.nouns.is_surely_plural(&text) || same_form_plural) {
                    continue;
                }
                let capital = starts_uppercase(words[at], document);
                lints.push(Lint {
                    span: words[at].span,
                    lint_kind: LintKind::Agreement,
                    suggestions: vec![Suggestion::ReplaceWith(
                        if capital { "Viele" } else { "viele" }.chars().collect(),
                    )],
                    message: format!("Vor »{text}« im Plural steht »viele«."),
                    priority: 31,
                });
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Setzt »viele« statt »viel« vor ein Nomen im Plural."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanVielPlural;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn count(text: &str) -> usize {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanVielPlural::default().lint(&document).len()
    }

    #[test]
    fn viel_before_a_plural_is_reported() {
        for text in [
            "Er hat viel Fehler gemacht.",
            "Viel Kinder spielen draußen.",
            "Wir haben viel Probleme.",
            "Ich habe viele Freunden.",
            "Alle Kindern spielen.",
        ] {
            assert_eq!(count(text), 1, "{text}");
        }
    }

    #[test]
    fn viel_before_a_singular_is_quiet() {
        for text in [
            "Er hat viele Fehler gemacht.",
            "Wir haben viel Zeit.",
            "Das ist viel Arbeit.",
            "Er trinkt viel Wasser.",
            "Das kostet zu viel Geld.",
            "Ich habe so viel Glück.",
            "Es gibt viel Neues.",
            "Er isst viel Fleisch.",
            "Ich habe viele Freunde.",
            "Wir helfen allen Kindern.",
            "Alle Menschen sind gleich.",
            "Viele Frauen kommen.",
        ] {
            assert_eq!(count(text), 0, "{text}");
        }
    }
}
