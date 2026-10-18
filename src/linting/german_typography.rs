//! German typography: units, dates and quotation marks.
//!
//! None of these is a matter of grammar, and all of them are cheap to see: the tokens are
//! punctuation and numbers, and what is wrong is how they are written next to each other.

use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Punctuation, Token, TokenKind};

fn text(document: &Document, token: &Token) -> String {
    document.get_span_content(&token.span).iter().collect()
}

fn lower(document: &Document, token: &Token) -> String {
    text(document, token).to_lowercase()
}

/// Units that are written with a space behind the number (Duden, § 65 of the rules of
/// spelling: *5 kg*, *10 MB*). Only the ones that cannot be anything else glued to a digit:
/// no single letters (*5m*, *3g*), no *min*, no *h*.
const UNITS: &[&str] = &[
    "kg", "km", "cm", "mm", "ml", "mg", "dl", "qm", "kb", "mb", "gb", "tb", "khz", "mhz", "ghz",
    "kw", "kwh", "mwh", "km/h", "€",
];

const MONTHS: &[&str] = &[
    "januar",
    "februar",
    "märz",
    "april",
    "mai",
    "juni",
    "juli",
    "august",
    "september",
    "oktober",
    "november",
    "dezember",
];

/// Words that put a date in the dative or accusative, so that the day is an ordinal number:
/// *am 5. Mai*, *bis zum 31. Dezember*, *vom 1. Januar*.
const BEFORE_A_DATE: &[&str] = &[
    "am", "vom", "zum", "bis", "seit", "ab", "den", "dem", "vor", "nach", "der", "zur", "im",
];

/// *5€*, *3kg*, *10MB*: a space between the number and its unit. And *am 5 Mai*: the point
/// behind the day.
#[derive(Default)]
pub struct GermanNumberFormat;

impl GermanNumberFormat {
    fn glued(a: &Token, b: &Token) -> bool {
        a.span.end == b.span.start
    }
}

impl Linter for GermanNumberFormat {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        let source = document.get_source();
        let tokens: Vec<&Token> = document
            .get_tokens()
            .iter()
            .filter(|token| !token.kind.is_whitespace())
            .collect();

        for (index, token) in tokens.iter().enumerate() {
            let TokenKind::Number(number) = &token.kind else {
                continue;
            };
            // A number inside an identifier or a version (*x5kg*, *v1.2.3*) is not a quantity.
            let preceded_by_a_letter = token
                .span
                .start
                .checked_sub(1)
                .and_then(|at| source.get(at))
                .is_some_and(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'));
            if preceded_by_a_letter {
                continue;
            }
            let Some(next) = tokens.get(index + 1) else {
                continue;
            };

            if Self::glued(token, next) {
                let unit = lower(document, next);
                let is_unit = match &next.kind {
                    TokenKind::Word(_) => UNITS.contains(&unit.as_str()),
                    TokenKind::Punctuation(Punctuation::Currency(_)) => unit == "€",
                    _ => false,
                };
                // *1990er*, *3D*, *5kg-Packung* are different things: a hyphen after the unit
                // makes it part of a compound.
                let compound = tokens
                    .get(index + 2)
                    .is_some_and(|after| Self::glued(next, after) && after.kind.is_hyphen());
                if is_unit && !compound {
                    lints.push(Lint {
                        span: token.span,
                        lint_kind: LintKind::Formatting,
                        suggestions: vec![Suggestion::InsertAfter(vec![' '])],
                        priority: 40,
                        message: format!(
                            "Zwischen Zahl und Einheit steht ein Leerzeichen: »{} {}«.",
                            text(document, token),
                            text(document, next)
                        ),
                    });
                }
            }

            // *am 5 Mai*: a whole number from 1 to 31, a month behind it, a preposition or an
            // article in front.
            let is_a_day = number.suffix.is_none()
                && number.precision == 0
                && (1.0..=31.0).contains(&number.value.into_inner());
            if !is_a_day || index == 0 {
                continue;
            }
            let month_follows = tokens.get(index + 1).is_some_and(|month| {
                matches!(month.kind, TokenKind::Word(_))
                    && MONTHS.contains(&lower(document, month).as_str())
                    && document
                        .get_span_content(&month.span)
                        .first()
                        .is_some_and(|c| c.is_uppercase())
            });
            let date_word_before =
                BEFORE_A_DATE.contains(&lower(document, tokens[index - 1]).as_str());
            if month_follows && date_word_before {
                lints.push(Lint {
                    span: token.span,
                    lint_kind: LintKind::Punctuation,
                    suggestions: vec![Suggestion::InsertAfter(vec!['.'])],
                    priority: 40,
                    message: "Hinter dem Tag steht ein Punkt: »5. Mai«.".to_owned(),
                });
            }
        }

        lints
    }

    fn description(&self) -> &str {
        "Setzt ein Leerzeichen zwischen Zahl und Einheit und den Punkt hinter den Tag im Datum."
    }
}

/// `"so"` and `“so”` next to `„so“`: one style in a document.
///
/// Straight quotes are what most German text on the web is typed with (about one sentence in
/// fifteen of a treebank of web and news text has a pair), so reporting each of them would bury
/// the document. What is wrong beyond doubt is a mix: where the writer has used `„…“` or `»…«`
/// somewhere, the straight and the English pair elsewhere are the slip. Each quotation mark is
/// reported on its own, one character wide, so that no other lint inside the quotation is
/// swallowed by an overlapping one.
#[derive(Default)]
pub struct GermanQuotationMarks;

impl Linter for GermanQuotationMarks {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        let source = document.get_source();
        let tokens = document.get_tokens();

        let german_style_used = source.iter().any(|c| matches!(c, '„' | '»' | '›'));
        if !german_style_used {
            return lints;
        }

        for (index, token) in tokens.iter().enumerate() {
            let TokenKind::Punctuation(Punctuation::Quote(quote)) = &token.kind else {
                continue;
            };
            // Only the opening quote of a pair starts a lint; the pair is found by the lexer.
            let Some(twin) = quote.twin_loc else {
                continue;
            };
            if twin <= index {
                continue;
            }
            let (open, close) = (token.span, tokens[twin].span);
            let opening = source[open.start];
            let closing = source[close.start];
            // The straight and the English curly pair are the wrong ones.
            if !matches!((opening, closing), ('"', '"') | ('“', '”')) {
                continue;
            }
            // Opens after a space or at the start, closes before a space or a sign: not a
            // measure (*5" Display*), not an attribute (*class="x"*), not a call (*print("x")*).
            let before_ok = open
                .start
                .checked_sub(1)
                .and_then(|at| source.get(at))
                .is_none_or(|c| c.is_whitespace());
            let after_ok = source
                .get(close.end)
                .is_none_or(|c| c.is_whitespace() || ".,;:!?)".contains(*c));
            let inner = &source[open.end..close.start];
            let spaced_inside = inner.first().is_some_and(|c| c.is_whitespace())
                || inner.last().is_some_and(|c| c.is_whitespace());
            if !before_ok || !after_ok || spaced_inside || inner.is_empty() || inner.len() > 300 {
                continue;
            }
            for (span, replacement) in [(open, '„'), (close, '“')] {
                lints.push(Lint {
                    span,
                    lint_kind: LintKind::Formatting,
                    suggestions: vec![Suggestion::ReplaceWith(vec![replacement])],
                    priority: 45,
                    message: "In diesem Text stehen die Anführungszeichen sonst so: „…“."
                        .to_owned(),
                });
            }
        }

        lints
    }

    fn description(&self) -> &str {
        "Ersetzt gerade und englische Anführungszeichen durch „…“, wenn der Text sonst deutsche verwendet."
    }
}

#[cfg(test)]
mod tests {
    use super::{GermanNumberFormat, GermanQuotationMarks};
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
    fn a_unit_is_written_apart() {
        for text in [
            "Das kostet 5€.",
            "Es wiegt 3kg.",
            "Die Datei hat 10MB.",
            "Bis zu 100km entfernt.",
        ] {
            assert_eq!(count(GermanNumberFormat, text), 1, "{text}");
        }
        for text in [
            "Das kostet 5 €.",
            "Es wiegt 3 kg.",
            "Eine 5kg-Packung.",
            "Der Spieler ist 5m groß.",
            "Version v1.5kg",
            "Die 1990er Jahre.",
            "Es gibt 3D-Drucker.",
        ] {
            assert_eq!(count(GermanNumberFormat, text), 0, "{text}");
        }
    }

    #[test]
    fn the_day_of_a_date_takes_a_point() {
        for text in [
            "Am 5 Mai ist Feiertag.",
            "Bis zum 31 Dezember.",
            "Das war vom 3 März.",
        ] {
            assert_eq!(count(GermanNumberFormat, text), 1, "{text}");
        }
        for text in [
            "Am 5. Mai ist Feiertag.",
            "Im Mai kamen 5 Gäste.",
            "Wir haben 5 Mai-Käfer.",
            "Das waren 40 Mai.",
        ] {
            assert_eq!(count(GermanNumberFormat, text), 0, "{text}");
        }
    }

    #[test]
    fn straight_and_english_quotes_follow_the_german_ones_of_the_text() {
        // Two lints per pair, one for each mark.
        for text in [
            "Er sagte „Das ist gut“ und \"so\" ging er.",
            "Er sagte „Das ist gut“ und “so” ging er.",
        ] {
            assert_eq!(count(GermanQuotationMarks, text), 2, "{text}");
        }
        for text in [
            // No German quotation mark anywhere: the writer's style, not a slip.
            "Er sagte \"Das ist gut\" und ging.",
            "Er sagte „Das ist gut“ und ging.",
            "Er sagte „Das“ und ein 5\" Display.",
            "Er sagte „Das“ und print(\"x\") rief auf.",
            "Er sagte „Das“ und <a class=\"x\">.",
        ] {
            assert_eq!(count(GermanQuotationMarks, text), 0, "{text}");
        }
    }
}
