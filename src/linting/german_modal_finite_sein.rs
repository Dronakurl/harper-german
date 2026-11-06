//! *Es kann ist, dass er kommt*, *Er sollte sind pünktlich*: a modal verb in
//! second place takes the infinitive *sein*, not a finite form of it.

use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Punctuation, Token, TokenKind, TokenStringExt};

use crate::grammar::noun_phrase::lowercase_of;
use crate::linting::german_subordinate_comma::SUBORDINATORS;

/// The finite forms of the six modals, present, preterite and subjunctive II.
const MODALS: &[&str] = &[
    "kann",
    "kannst",
    "können",
    "könnt",
    "konnte",
    "konntest",
    "konnten",
    "konntet",
    "könnte",
    "könntest",
    "könnten",
    "könntet",
    "muss",
    "musst",
    "müssen",
    "müsst",
    "musste",
    "musstest",
    "mussten",
    "musstet",
    "müsste",
    "müsstest",
    "müssten",
    "müsstet",
    "will",
    "willst",
    "wollen",
    "wollt",
    "wollte",
    "wolltest",
    "wollten",
    "wolltet",
    "soll",
    "sollst",
    "sollen",
    "sollt",
    "sollte",
    "solltest",
    "sollten",
    "solltet",
    "darf",
    "darfst",
    "dürfen",
    "dürft",
    "durfte",
    "durftest",
    "durften",
    "durftet",
    "dürfte",
    "dürftest",
    "dürften",
    "dürftet",
    "mag",
    "magst",
    "mögen",
    "mögt",
    "mochte",
    "mochtest",
    "mochten",
    "mochtet",
    "möchte",
    "möchtest",
    "möchten",
    "möchtet",
];

/// The finite present of *sein*.
const FINITE_SEIN: &[&str] = &["ist", "sind", "bin", "bist", "seid"];

/// Words that make the clause a subordinate one, with the verb at its end: *was
/// er kann ist toll*, *wer kann ist besser dran*. There the comma is missing,
/// not the infinitive.
const CLAUSE_OPENERS: &[&str] = &[
    "was", "wer", "wen", "wem", "wessen", "wie", "wo", "wohin", "woher", "warum", "weshalb",
    "wieso", "wann", "welche", "welcher", "welches", "welchen", "wenn", "als", "ob", "je",
];

/// Relative pronouns, which open a clause only behind a comma: *der Mann, der
/// kommen soll ist da*. At the start of the sentence they are the article.
const RELATIVES: &[&str] = &[
    "der", "die", "das", "dem", "den", "deren", "dessen", "denen",
];

/// `ModalIst.weir` already reports these before *ist*, anywhere.
const WEIR_PAIRS: &[&str] = &["könnte", "müsste", "dürfte", "sollte"];

/// A modal in second place followed by a finite *sein*. The clause in front of
/// the modal has to be a main clause: nothing from [`CLAUSE_OPENERS`] or
/// [`SUBORDINATORS`] between its start (the sentence start or a comma) and the
/// modal, and at most three words, so the modal stands in second place.
#[derive(Default)]
pub struct GermanModalFiniteSein;

impl Linter for GermanModalFiniteSein {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|t| !t.kind.is_whitespace())
                .collect();
            for at in 1..words.len().saturating_sub(1) {
                let modal = lowercase_of(words[at], document);
                let sein = lowercase_of(words[at + 1], document);
                if !MODALS.contains(&modal.as_str())
                    || !FINITE_SEIN.contains(&sein.as_str())
                    || (sein == "ist" && WEIR_PAIRS.contains(&modal.as_str()))
                {
                    continue;
                }
                // Lower case: a finite *ist* typed by mistake, not *Ist*.
                if document
                    .get_span_content(&words[at + 1].span)
                    .first()
                    .is_some_and(|c| c.is_uppercase())
                {
                    continue;
                }
                let clause_start = words[..at]
                    .iter()
                    .rposition(|t| {
                        matches!(
                            t.kind,
                            TokenKind::Punctuation(
                                Punctuation::Comma | Punctuation::Semicolon | Punctuation::Colon
                            )
                        )
                    })
                    .map_or(0, |i| i + 1);
                let front = &words[clause_start..at];
                if front.is_empty()
                    || front.len() > 3
                    || front.iter().any(|t| {
                        let word = lowercase_of(t, document);
                        !matches!(t.kind, TokenKind::Word(_))
                            || CLAUSE_OPENERS.contains(&word.as_str())
                            || SUBORDINATORS.contains(&word.as_str())
                            || (clause_start > 0 && RELATIVES.contains(&word.as_str()))
                    })
                {
                    continue;
                }
                // The infinitive replaces the finite form where it stands when
                // nothing else follows in the clause (*Es kann ist, dass …*);
                // otherwise it belongs at the end (*Er sollte pünktlich sein*).
                let clause_ends_here = words.get(at + 2).is_none_or(|t| {
                    matches!(
                        t.kind,
                        TokenKind::Punctuation(
                            Punctuation::Comma | Punctuation::Period | Punctuation::Bang
                        )
                    )
                });
                let written = document.get_span_content_str(&words[at + 1].span);
                lints.push(Lint {
                    span: words[at + 1].span,
                    lint_kind: LintKind::Grammar,
                    suggestions: if clause_ends_here {
                        vec![Suggestion::ReplaceWith("sein".chars().collect())]
                    } else {
                        Vec::new()
                    },
                    priority: 31,
                    message: if clause_ends_here {
                        format!("Nach einem Modalverb steht der Infinitiv: »sein«, nicht »{written}«.")
                    } else {
                        format!(
                            "Nach einem Modalverb steht der Infinitiv »sein«, am Ende des Satzes: »… pünktlich sein«, nicht »{written}«."
                        )
                    },
                });
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Findet eine finite Form von »sein« hinter einem Modalverb: »Es kann ist« statt »Es kann sein«."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanModalFiniteSein;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn reported(text: &str) -> Vec<String> {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanModalFiniteSein
            .lint(&document)
            .iter()
            .map(|l| document.get_span_content_str(&l.span))
            .collect()
    }

    #[test]
    fn a_finite_sein_after_a_modal_is_reported() {
        assert_eq!(reported("Es kann ist, dass er kommt."), ["ist"]);
        assert_eq!(reported("Er sollte sind pünktlich."), ["sind"]);
        assert_eq!(reported("Wir müssen sind vorsichtig."), ["sind"]);
        assert_eq!(reported("Das Kind kann bin müde."), ["bin"]);
    }

    #[test]
    fn a_missing_comma_is_left_alone() {
        for text in [
            "Wer kann ist besser dran.",
            "Was er kann ist toll.",
            "Wenn er will ist er schnell.",
            "Ich weiß, dass er kann ist klar.",
            "Es könnte ist, dass ich nicht gut bin.",
            "Es kann sein, dass er kommt.",
            "Er sollte pünktlich sein.",
            "Der Mann, der kommen soll ist da.",
            "Wer ein \"echtes\" Steak essen möchte ist hier richtig!",
        ] {
            assert!(reported(text).is_empty(), "{text}: {:?}", reported(text));
        }
    }
}
