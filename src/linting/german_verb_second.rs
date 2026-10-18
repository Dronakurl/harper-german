//! The finite verb of a main clause stands in second place.

use std::sync::Arc;

use crate::grammar::subjects::subject_pronoun;
use crate::linting::german_subject_verb_agreement::GermanSubjectVerbAgreement;
use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::spell::Dictionary;
use harper_core::{Document, Token, TokenStringExt};

/// Adverbs that open a main clause and nothing else: after one of them the verb comes before the
/// subject. A closed list on purpose. *Heute ich gehe* is the mistake of everyone who thinks in
/// English word order, and a list of the adverbs people actually put first finds it without having
/// to parse the sentence.
///
/// Left out: every word that is also a conjunction or opens a subordinate clause (*wenn*, *als*,
/// *dass*, *trotzdem* in the colloquial use, *zumal*), and *immer*, *nur*, *auch*, which sit in
/// front of a pronoun in set phrases (*nur ich*, *auch ich*).
const OPENERS: &[&str] = &[
    "heute",
    "gestern",
    "morgen",
    "jetzt",
    "dann",
    "danach",
    "später",
    "früher",
    "hier",
    "dort",
    "manchmal",
    "oft",
    "zuerst",
    "zuletzt",
    "leider",
    "vielleicht",
    "natürlich",
    "außerdem",
    "deshalb",
    "deswegen",
    "daher",
    "plötzlich",
    "schließlich",
    "normalerweise",
    "übrigens",
];

/// *Heute ich gehe ins Kino.* → *Heute gehe ich ins Kino.*
///
/// The check is local and strict: the sentence starts with one of [`OPENERS`], a personal pronoun
/// follows, and behind the pronoun comes a word that is a finite verb **agreeing with it**
/// (`verb_features` of the agreement rule, so *ich gehen* is left to that rule). A comma after the
/// adverb, a conjunction, anything else in between and the whole check does not apply.
pub struct GermanVerbSecond<T: Dictionary> {
    agreement: GermanSubjectVerbAgreement<T>,
}

impl<T: Dictionary> GermanVerbSecond<T> {
    pub fn new(dictionary: Arc<T>) -> Self {
        Self {
            agreement: GermanSubjectVerbAgreement::new(dictionary),
        }
    }
}

impl<T: Dictionary> Linter for GermanVerbSecond<T> {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let tokens: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            let [opener, pronoun, verb, ..] = tokens.as_slice() else {
                continue;
            };
            let text = |token: &Token| -> String {
                document.get_span_content(&token.span).iter().collect()
            };
            if !matches!(opener.kind, harper_core::TokenKind::Word(_))
                || !matches!(pronoun.kind, harper_core::TokenKind::Word(_))
                || !matches!(verb.kind, harper_core::TokenKind::Word(_))
                || !OPENERS.contains(&text(opener).to_lowercase().as_str())
            {
                continue;
            }
            // Capitalized, as at the start of a sentence; a lower-case opener is a fragment.
            if !text(opener).chars().next().is_some_and(char::is_uppercase) {
                continue;
            }
            let Some(subject) = subject_pronoun(&text(pronoun)) else {
                continue;
            };
            let verb_text = text(verb);
            let readings = self.agreement.verb_features(&verb_text, verb);
            if readings.is_empty() || !readings.iter().any(|r| subject.agrees_with(r)) {
                continue;
            }
            let swapped: Vec<char> = format!("{verb_text} {}", text(pronoun)).chars().collect();
            lints.push(Lint {
                span: harper_core::Span::new(pronoun.span.start, verb.span.end),
                lint_kind: LintKind::Grammar,
                suggestions: vec![Suggestion::ReplaceWith(swapped)],
                priority: 29,
                message: format!(
                    "Im Hauptsatz steht das Verb an zweiter Stelle: »{} {verb_text} {}«.",
                    text(opener),
                    text(pronoun)
                ),
            });
        }
        lints
    }

    fn description(&self) -> &str {
        "Prüft, ob das Verb nach einem einleitenden Adverb an zweiter Stelle steht."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanVerbSecond;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn count(text: &str) -> usize {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanVerbSecond::new(dict.clone()).lint(&document).len()
    }

    #[test]
    fn the_verb_comes_before_the_subject_after_an_adverb() {
        for text in [
            "Heute ich gehe ins Kino.",
            "Gestern wir haben Fußball gespielt.",
            "Dann er kommt nach Hause.",
            "Leider ich habe keine Zeit.",
        ] {
            assert_eq!(count(text), 1, "{text}");
        }
    }

    #[test]
    fn the_right_order_and_the_other_cases_are_quiet() {
        for text in [
            "Heute gehe ich ins Kino.",
            "Gestern haben wir Fußball gespielt.",
            "Heute, ich weiß, ist es kalt.",
            "Heute ich gehen ins Kino.",
            "Dann ich.",
            "Jetzt ich und du.",
            "Hier ich bin nicht gemeint, sagt er.",
        ] {
            let expected = usize::from(text.starts_with("Hier"));
            assert_eq!(count(text), expected, "{text}");
        }
    }
}
