//! *Wann du kommst morgen?*, *Kannst du mir sagen, wo ist der Bahnhof?*, *Es regnete,
//! trotzdem wir gingen raus.*: the finite verb in the wrong place.

use harper_core::{
    Document, Punctuation, Span, Token, TokenKind, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
};

use crate::grammar::noun_phrase::lowercase_of;
use crate::grammar::subjects::irregular_finite_verb_lemma;
use crate::grammar::token_text::starts_lowercase;
use crate::linting::german_relative_clause_comma::GermanRelativeClauseComma;

const QUESTION_WORDS: &[&str] = &[
    "wann", "was", "warum", "wo", "wie", "wohin", "woher", "weshalb", "wieso", "weswegen",
];

const PERSONAL_PRONOUNS: &[&str] = &["ich", "du", "er", "sie", "es", "wir", "ihr"];

/// Adverbs that open a main clause and take the verb right behind them: *Es regnete, trotzdem
/// gingen wir raus*.
const VERB_SECOND_ADVERBS: &[&str] = &[
    "trotzdem",
    "deshalb",
    "deswegen",
    "darum",
    "daher",
    "danach",
    "dann",
    "also",
    "dennoch",
    "außerdem",
    "anschließend",
    "sonst",
];

/// The verbs of a main clause that an indirect question depends on.
const ASKING_VERBS: &[&str] = &[
    "sagen",
    "sag",
    "sagst",
    "sagt",
    "weißt",
    "wisst",
    "wissen",
    "weiß",
    "frage",
    "fragt",
    "fragst",
    "fragen",
    "erklären",
    "erklär",
    "erklärst",
    "zeigen",
    "zeig",
    "zeigst",
    "verraten",
];

fn ends_with_question_mark(words: &[&Token]) -> bool {
    words
        .last()
        .is_some_and(|t| matches!(t.kind, TokenKind::Punctuation(Punctuation::Question)))
}

fn is_finite_verb(token: &Token, document: &Document) -> bool {
    if !starts_lowercase(token, document) || !token.kind.is_word() {
        return false;
    }
    let word = lowercase_of(token, document);
    irregular_finite_verb_lemma(&word).is_some()
        || (GermanRelativeClauseComma::is_verb_token(token, document)
            && !token.kind.is_adjective()
            && !token.kind.is_noun()
            && !token.kind.is_determiner()
            && !token.kind.is_pronoun()
            && !token.kind.is_preposition()
            && !token.kind.is_adverb()
            && (word.ends_with('t')
                || word.ends_with("st")
                || word.ends_with('e')
                || word.ends_with('n')))
}

/// Reports a finite verb that stands where the clause does not allow it:
///
/// * behind a question word and a personal pronoun in a direct question: *Wann du kommst
///   morgen?* → *kommst du*;
/// * right behind the question word of an indirect question at the end of a sentence whose main
///   clause asks or tells: *Kannst du mir sagen, wo ist der Bahnhof?* → *wo der Bahnhof ist*;
/// * behind a personal pronoun after an adverb that opens a clause: *Es regnete, trotzdem wir
///   gingen raus* → *trotzdem gingen wir*.
#[derive(Default)]
pub struct GermanQuestionWordOrder;

impl GermanQuestionWordOrder {
    fn swap(first: &Token, second: &Token, document: &Document, message: &str) -> Lint {
        let a = document.get_span_content_str(&first.span);
        let b = document.get_span_content_str(&second.span);
        Lint {
            span: Span::new(first.span.start, second.span.end),
            lint_kind: LintKind::WordOrder,
            suggestions: vec![Suggestion::ReplaceWith(
                format!("{b} {a}").chars().collect(),
            )],
            message: message.to_owned(),
            priority: 31,
        }
    }
}

impl Linter for GermanQuestionWordOrder {
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
            let question = ends_with_question_mark(&words);
            let lower: Vec<String> = words.iter().map(|t| lowercase_of(t, document)).collect();

            // *Wann du kommst morgen?*
            if question
                && QUESTION_WORDS.contains(&lower[0].as_str())
                && PERSONAL_PRONOUNS.contains(&lower[1].as_str())
                && is_finite_verb(words[2], document)
                // *Was du nicht sagst?* has no verb in the third place.
                && !words[3..].iter().any(|t| t.kind.is_comma())
            {
                lints.push(Self::swap(
                    words[1],
                    words[2],
                    document,
                    "In einer Frage mit Fragewort steht das Verb direkt hinter dem Fragewort.",
                ));
            }

            // *Trotzdem es regnete, gingen wir raus*: a subordinate clause needs *obwohl*;
            // *trotzdem* opens a main clause with its verb second.
            if lower[0] == "trotzdem"
                && !is_finite_verb(words[1], document)
                && !irregular_finite_verb_lemma(&lower[1]).is_some()
                && let Some(comma) = words.iter().position(|t| t.kind.is_comma())
                && comma >= 3
                && is_finite_verb(words[comma - 1], document)
                && words.get(comma + 1).is_some_and(|t| {
                    irregular_finite_verb_lemma(&lowercase_of(t, document)).is_some()
                        || GermanRelativeClauseComma::is_verb_token(t, document)
                })
            {
                lints.push(Lint {
                    span: words[0].span,
                    lint_kind: LintKind::WordChoice,
                    suggestions: vec![Suggestion::replace_with_match_case(
                        "obwohl".chars().collect(),
                        document.get_span_content(&words[0].span),
                    )],
                    message:
                        "Einen Nebensatz leitet »obwohl« ein; »trotzdem« eröffnet einen Hauptsatz."
                            .to_owned(),
                    priority: 31,
                });
            }

            // *…, dass er morgen kommt nicht.*: in a verb-final clause *nicht* stands before the
            // verb.
            let mut clause_start = 0;
            for at in 0..words.len() {
                if words[at].kind.is_comma() {
                    clause_start = at + 1;
                    continue;
                }
                let ends = words.get(at + 1).is_none_or(|t| {
                    matches!(t.kind, TokenKind::Punctuation(_)) && !t.kind.is_hyphen()
                });
                if !ends || at < clause_start + 3 || lower[at] != "nicht" {
                    continue;
                }
                let opener = lower[clause_start].as_str();
                if !matches!(
                    opener,
                    "dass"
                        | "weil"
                        | "wenn"
                        | "ob"
                        | "obwohl"
                        | "als"
                        | "damit"
                        | "bevor"
                        | "nachdem"
                        | "falls"
                        | "sodass"
                ) {
                    continue;
                }
                let verb = words[at - 1];
                if !is_finite_verb(verb, document)
                    || !PERSONAL_PRONOUNS.contains(&lower[clause_start + 1].as_str())
                {
                    continue;
                }
                lints.push(Self::swap(
                    verb,
                    words[at],
                    document,
                    "Im Nebensatz steht das Verb am Ende, hinter »nicht«.",
                ));
            }

            for at in 1..words.len().saturating_sub(3) {
                if !words[at - 1].kind.is_comma() {
                    continue;
                }
                // *…, trotzdem wir gingen raus.*
                if VERB_SECOND_ADVERBS.contains(&lower[at].as_str())
                    && PERSONAL_PRONOUNS.contains(&lower[at + 1].as_str())
                    && is_finite_verb(words[at + 2], document)
                {
                    lints.push(Self::swap(
                        words[at + 1],
                        words[at + 2],
                        document,
                        "Nach dem Adverb am Anfang des Satzes steht das Verb an zweiter Stelle.",
                    ));
                }

                // *Kannst du mir sagen, wo ist der Bahnhof?*
                if QUESTION_WORDS.contains(&lower[at].as_str())
                    && is_finite_verb(words[at + 1], document)
                    && irregular_finite_verb_lemma(&lower[at + 1]).is_some()
                    && lower[..at - 1]
                        .iter()
                        .any(|word| ASKING_VERBS.contains(&word.as_str()))
                    && !words[at + 1..].iter().any(|t| t.kind.is_comma())
                    // A question mark belongs to the main clause only when that is a question:
                    // *Kannst du mir sagen, wo …?*; in *Er fragt: Nun, was hast du gemacht?* the
                    // question is direct speech.
                    && (!question || is_finite_verb(words[0], document) || {
                        let first = document.get_span_content_str(&words[0].span);
                        ["Kannst", "Können", "Könnten", "Weißt", "Wissen", "Wisst", "Sag", "Sagst"]
                            .contains(&first.as_str())
                    })
                    && !words[..at].iter().any(|t| {
                        matches!(t.kind, TokenKind::Punctuation(Punctuation::Colon))
                    })
                {
                    let end = words.len() - 1;
                    if !matches!(words[end].kind, TokenKind::Punctuation(_)) || end <= at + 2 {
                        continue;
                    }
                    let verb = document.get_span_content_str(&words[at + 1].span);
                    let rest_span = Span::new(words[at + 2].span.start, words[end - 1].span.end);
                    let rest = document.get_span_content_str(&rest_span);
                    lints.push(Lint {
                        span: Span::new(words[at + 1].span.start, words[end - 1].span.end),
                        lint_kind: LintKind::WordOrder,
                        suggestions: vec![Suggestion::ReplaceWith(
                            format!("{rest} {verb}").chars().collect(),
                        )],
                        message: "In einer indirekten Frage steht das Verb am Ende.".to_owned(),
                        priority: 31,
                    });
                }
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Stellt das Verb in Fragen mit Fragewort, in indirekten Fragen und nach »trotzdem« an die richtige Stelle."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanQuestionWordOrder;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn fixes(text: &str) -> Vec<String> {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanQuestionWordOrder
            .lint(&document)
            .into_iter()
            .flat_map(|lint| lint.suggestions)
            .map(|suggestion| suggestion.to_string())
            .collect()
    }

    #[test]
    fn the_verb_in_the_wrong_place_is_reported() {
        for (text, fixed) in [
            ("Wann du kommst morgen?", "kommst du"),
            ("Was du machst heute?", "machst du"),
            ("Warum er hat das gemacht?", "hat er"),
            ("Es regnete, trotzdem wir gingen raus.", "gingen wir"),
            ("Trotzdem es regnete, gingen wir raus.", "Obwohl"),
            ("Er sagt, dass er morgen kommt nicht.", "nicht kommt"),
            (
                "Kannst du mir sagen, wo ist der Bahnhof?",
                "der Bahnhof ist",
            ),
        ] {
            assert_eq!(fixes(text), [format!("Replace with: “{fixed}”")], "{text}");
        }
    }

    #[test]
    fn right_orders_are_quiet() {
        for text in [
            "Wann kommst du morgen?",
            "Kannst du mir sagen, wo der Bahnhof ist?",
            "Es regnete, trotzdem gingen wir raus.",
            "Wie du meinst.",
            "Ich weiß, wo er wohnt.",
            "Was du nicht sagst!",
            "Er sagt, dann kommt er.",
            "Wo ist der Bahnhof?",
            "Trotzdem gingen wir raus, weil es schön war.",
            "Trotzdem kam er, als es regnete.",
            "Er sagt, dass er morgen nicht kommt.",
            "Er kommt nicht.",
            "Wenn du willst, komm nicht.",
        ] {
            assert!(fixes(text).is_empty(), "{text}: {:?}", fixes(text));
        }
    }
}
