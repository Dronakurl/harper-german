//! Wordings that are wrong although every word is a word: *Der Grund ist, weil …* and *am
//! bestesten*.

use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Punctuation, Token, TokenKind, TokenStringExt};

fn word(document: &Document, token: &Token) -> String {
    document
        .get_span_content(&token.span)
        .iter()
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn is_word(token: &Token) -> bool {
    matches!(token.kind, TokenKind::Word(_))
}

fn sentence_tokens(sentence: &[Token]) -> Vec<&Token> {
    sentence
        .iter()
        .filter(|token| !token.kind.is_whitespace())
        .collect()
}

/// *Der Grund ist, weil er krank ist.* asks for *dass* (Duden): the noun already names the
/// reason, so what follows the copula is a content clause. Spoken German accepts *weil* with the
/// verb second (`GermanSubordinateWordOrder` leaves that alone); this is a different mistake,
/// because the construction wants a *dass*-clause.
///
/// Only a capitalized *Grund*, *Ursache* or *Begründung*, directly in front of *ist* or *war*
/// (with *dafür* between them at most) and the comma: *Der Grund war gut, weil …* and *Das ist
/// der Grund, weil …* have other words in between and are left as they are.
#[derive(Default)]
pub struct GermanReasonIsBecause;

impl Linter for GermanReasonIsBecause {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let tokens = sentence_tokens(sentence);
            for (index, token) in tokens.iter().enumerate() {
                let capitalized = document
                    .get_span_content(&token.span)
                    .first()
                    .is_some_and(|c| c.is_uppercase());
                if !is_word(token)
                    || !capitalized
                    || !["grund", "ursache", "begründung"].contains(&word(document, token).as_str())
                {
                    continue;
                }
                let mut at = index + 1;
                if tokens
                    .get(at)
                    .is_some_and(|t| is_word(t) && word(document, t) == "dafür")
                {
                    at += 1;
                }
                let copula = tokens.get(at).is_some_and(|t| {
                    is_word(t) && ["ist", "war"].contains(&word(document, t).as_str())
                });
                let comma = tokens
                    .get(at + 1)
                    .is_some_and(|t| matches!(t.kind, TokenKind::Punctuation(Punctuation::Comma)));
                let Some(weil) = tokens.get(at + 2).filter(|t| is_word(t)) else {
                    continue;
                };
                if copula && comma && word(document, weil) == "weil" {
                    lints.push(Lint {
                        span: weil.span,
                        lint_kind: LintKind::Grammar,
                        suggestions: vec![Suggestion::ReplaceWith(vec!['d', 'a', 's', 's'])],
                        priority: 31,
                        message: "Nach »Der Grund ist,« folgt »dass«, nicht »weil«.".to_owned(),
                    });
                }
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Ersetzt »weil« in »Der Grund ist, weil …« durch »dass«."
    }
}

/// Comparatives and superlatives that are marked twice: *mehr besser*, *am bestesten*.
///
/// * *am bestesten* → *am besten*: the superlative ending *-sten* after *am* is complete, and a
///   second *-es-* never occurs: no German adjective has a superlative in *-stesten*.
/// * *mehr besser* → *besser*, for the irregular comparatives that cannot take *mehr* (*besser*,
///   *lieber*, *höher*, ...). Not after a negation, *immer*, *viel* or *noch*, where *mehr* is a
///   quantity or a time: *nicht mehr besser* means "no longer better".
#[derive(Default)]
pub struct GermanDoubleComparison;

const IRREGULAR_COMPARATIVES: &[&str] = &[
    "besser",
    "lieber",
    "höher",
    "näher",
    "größer",
    "kleiner",
    "älter",
    "jünger",
    "schöner",
    "schlechter",
    "wichtiger",
    "stärker",
    "schwächer",
    "länger",
    "kürzer",
    "schneller",
    "weniger",
];

/// What in front of *mehr* makes it a quantity or a time rather than part of a comparison.
const NOT_BEFORE_MEHR: &[&str] = &[
    "nicht", "nie", "niemals", "kein", "keine", "keinen", "keiner", "kaum", "immer", "viel",
    "noch", "ein", "sehr", "gar", "nichts", "nur", "umso", "je", "desto", "und", "oder", "als",
    "wie", "was", "wer", "wenn", "ob",
];

impl Linter for GermanDoubleComparison {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let tokens = sentence_tokens(sentence);
            for (index, token) in tokens.iter().enumerate() {
                if !is_word(token) {
                    continue;
                }
                let text = word(document, token);

                // *am bestesten*
                if index > 0
                    && is_word(tokens[index - 1])
                    && word(document, tokens[index - 1]) == "am"
                    && text.chars().count() > 7
                    && let Some(stem) = text.strip_suffix("esten")
                    && stem.ends_with("st")
                {
                    let fixed: Vec<char> = format!("{stem}en").chars().collect();
                    lints.push(Lint {
                        span: token.span,
                        lint_kind: LintKind::Grammar,
                        suggestions: vec![Suggestion::ReplaceWith(fixed)],
                        priority: 31,
                        message: "Der Superlativ ist schon vollständig. »-esten« steht doppelt."
                            .to_owned(),
                    });
                }

                // *mehr besser*
                if text == "mehr"
                    && let Some(next) = tokens.get(index + 1).filter(|t| is_word(t))
                    && IRREGULAR_COMPARATIVES.contains(&word(document, next).as_str())
                    && word(document, next) != "weniger"
                    && !(index > 0 && {
                        let before = word(document, tokens[index - 1]);
                        is_word(tokens[index - 1]) && NOT_BEFORE_MEHR.contains(&before.as_str())
                    })
                    && index > 0
                {
                    let replacement: Vec<char> = document.get_span_content(&next.span).to_vec();
                    lints.push(Lint {
                        span: harper_core::Span::new(token.span.start, next.span.end),
                        lint_kind: LintKind::Grammar,
                        suggestions: vec![Suggestion::ReplaceWith(replacement)],
                        priority: 31,
                        message: "Der Komparativ ist schon gesteigert. »mehr« ist zu viel."
                            .to_owned(),
                    });
                }
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Streicht die doppelte Steigerung in »mehr besser« und »am bestesten«."
    }
}

/// *Das Haus wurde gebaut worden.* The passive has its auxiliary once: *wurde gebaut* or *ist
/// gebaut worden*. *wurde* and *worden* in one clause is the two forms mixed.
///
/// Only *wurde* and *wurden*, and *worden* within the next three words, with no comma, *und*,
/// *oder* or *sein* in between or behind: *wird gebaut worden sein* is the future perfect and
/// right, and a comma ends the clause.
#[derive(Default)]
pub struct GermanDoublePassive;

impl Linter for GermanDoublePassive {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let tokens = sentence_tokens(sentence);
            for (index, token) in tokens.iter().enumerate() {
                if !is_word(token) || !["wurde", "wurden"].contains(&word(document, token).as_str())
                {
                    continue;
                }
                for offset in 1..=4 {
                    let Some(candidate) = tokens.get(index + offset) else {
                        break;
                    };
                    if !is_word(candidate) {
                        break;
                    }
                    let text = word(document, candidate);
                    if ["und", "oder", "sowie", "dass", "weil", "wenn", "ob"]
                        .contains(&text.as_str())
                    {
                        break;
                    }
                    if text == "worden" {
                        let followed_by_sein = tokens
                            .get(index + offset + 1)
                            .is_some_and(|t| is_word(t) && word(document, t).starts_with("sei"));
                        if !followed_by_sein {
                            lints.push(Lint {
                                span: candidate.span,
                                lint_kind: LintKind::Grammar,
                                suggestions: vec![Suggestion::Remove],
                                priority: 31,
                                message: "»wurde« und »worden« stehen zusammen im Passiv nicht nebeneinander: entweder »wurde gebaut« oder »ist gebaut worden«.".to_owned(),
                            });
                        }
                        break;
                    }
                }
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Meldet »wurde … worden« in einem Satz."
    }
}

#[cfg(test)]
mod tests {
    use super::{GermanDoubleComparison, GermanDoublePassive, GermanReasonIsBecause};
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn count(mut linter: impl Linter, text: &str) -> usize {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        linter.lint(&document).len()
    }

    #[test]
    fn the_reason_wants_dass() {
        for text in [
            "Der Grund ist, weil er krank ist.",
            "Der Grund war, weil es regnete.",
            "Die Ursache ist, weil das Kabel fehlt.",
            "Der Grund dafür ist, weil er krank ist.",
        ] {
            assert_eq!(count(GermanReasonIsBecause, text), 1, "{text}");
        }
        for text in [
            "Der Grund ist, dass er krank ist.",
            "Er kommt nicht, weil er krank ist.",
            "Der Grund war gut, weil er ehrlich war.",
            "Das ist der Grund, weil ich es gesagt habe.",
        ] {
            assert_eq!(count(GermanReasonIsBecause, text), 0, "{text}");
        }
    }

    #[test]
    fn a_comparison_is_not_marked_twice() {
        for text in [
            "Das ist am bestesten.",
            "Er war am meistesten begeistert.",
            "Das ist mehr besser als vorher.",
            "Es wird mehr schöner.",
        ] {
            assert_eq!(count(GermanDoubleComparison, text), 1, "{text}");
        }
        for text in [
            "Das ist am besten.",
            "Er war am meisten begeistert.",
            "Das wird nicht mehr besser.",
            "Es wird immer mehr besser.",
            "Wir brauchen mehr Besseres.",
            "Das ist mehr als besser.",
        ] {
            assert_eq!(count(GermanDoubleComparison, text), 0, "{text}");
        }
    }

    #[test]
    fn the_passive_has_its_auxiliary_once() {
        for text in [
            "Das Haus wurde gebaut worden.",
            "Die Häuser wurden vor Jahren verkauft worden.",
        ] {
            assert_eq!(count(GermanDoublePassive, text), 1, "{text}");
        }
        for text in [
            "Das Haus wurde gebaut.",
            "Das Haus ist gebaut worden.",
            "Das Haus wird gebaut worden sein.",
            "Das Haus wurde gebaut, nachdem es verkauft worden war.",
            "Das Haus wurde gebaut und verkauft worden ist es nie.",
        ] {
            assert_eq!(count(GermanDoublePassive, text), 0, "{text}");
        }
    }
}
