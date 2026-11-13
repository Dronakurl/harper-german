//! *Er hat viel Fehler gemacht*: in front of a plural, *viel* is *viele*.

use harper_core::{
    Document, Token, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
};

use crate::grammar::noun_gender::NounGender;
use crate::grammar::noun_phrase::lowercase_of;
use crate::grammar::token_text::starts_uppercase;
use crate::morphology::GenderSet;

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

impl Linter for GermanVielPlural {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            for at in 0..words.len().saturating_sub(1) {
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
        ] {
            assert_eq!(count(text), 0, "{text}");
        }
    }
}
