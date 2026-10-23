//! Two nouns written apart that the dictionary has as one word: *Kaffee Maschine*.

use std::sync::Arc;

use crate::grammar::determiners::determiner_readings;
use crate::grammar::noun_gender::NounGender;
use crate::morphology::GenderSet;
use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::spell::Dictionary;
use harper_core::{Document, Token, TokenKind, TokenStringExt};

/// The "Deppenleerzeichen": *Kaffee Maschine*, *Computer Bildschirm*. German writes a compound noun
/// as one word, and the English habit of a space between the parts turns up in every text.
///
/// The lessons of the compound checker are kept: only a **dictionary entry** counts as a compound.
/// A word the checker builds from parts would accept almost any pair (*Papier Stift* →
/// *Papierstift*), and so would turn two ordinary neighbouring nouns into an error. Both parts
/// must be capitalized nouns that are not names, one plain space apart; the whole must be a noun
/// entry, with nothing, an *-s-* or an *-n-/-en-* between the parts. A sentence in which most
/// words are capitalized is a title and is left alone.
/// Days and months, which take a time of day or a number as a word of their own.
const TIME_WORDS: &[&str] = &[
    "Montag",
    "Dienstag",
    "Mittwoch",
    "Donnerstag",
    "Freitag",
    "Samstag",
    "Sonntag",
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
];

/// Words that make the noun behind them a plural of its own: *viele Tage Urlaub*.
const QUANTITIES: &[&str] = &[
    "viele", "mehrere", "einige", "wenige", "zwei", "drei", "vier", "fünf", "sechs", "sieben",
    "acht", "neun", "zehn", "zwölf", "hundert", "tausend", "alle", "beide", "manche",
];

pub struct GermanSplitCompound<T: Dictionary> {
    entries: Arc<T>,
    nouns: NounGender,
}

impl<T: Dictionary> GermanSplitCompound<T> {
    /// `entries` is the dictionary **without** compound building.
    pub fn new(entries: Arc<T>) -> Self {
        Self {
            entries,
            nouns: NounGender::new(),
        }
    }

    fn noun(token: &Token) -> bool {
        matches!(&token.kind, TokenKind::Word(Some(meta)) if meta.is_noun() && !meta.is_proper_noun())
    }
}

impl<T: Dictionary> Linter for GermanSplitCompound<T> {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        let source = document.get_source();

        for sentence in document.iter_sentences() {
            let tokens: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            let words = tokens
                .iter()
                .filter(|t| matches!(t.kind, TokenKind::Word(_)))
                .count();
            let capitalized = tokens
                .iter()
                .filter(|t| {
                    matches!(t.kind, TokenKind::Word(_)) && source[t.span.start].is_uppercase()
                })
                .count();
            // A title (most words capitalized) or a fragment (a name, a heading) is not prose.
            if words < 4 || capitalized * 10 >= words * 7 {
                continue;
            }

            for (at, pair) in tokens.windows(2).enumerate() {
                let (first, second) = (pair[0], pair[1]);
                // A number, a quantity or another noun in front means the first noun is a plural
                // or an object of its own: *2 Jahre Zeit*, *Millionen Menschen Strom*.
                if at > 0 && {
                    let before = tokens[at - 1];
                    matches!(before.kind, TokenKind::Number(_))
                        || Self::noun(before)
                        || QUANTITIES.contains(
                            &document
                                .get_span_content(&before.span)
                                .iter()
                                .flat_map(|c| c.to_lowercase())
                                .collect::<String>()
                                .as_str(),
                        )
                } {
                    continue;
                }
                // *der Liebe Gott*: an adjective written with a capital is not the start of a
                // compound.
                if matches!(&first.kind, TokenKind::Word(Some(meta)) if meta.is_adjective()) {
                    continue;
                }
                if !Self::noun(first)
                    || !Self::noun(second)
                    || first.span.end + 1 != second.span.start
                    || source[first.span.end] != ' '
                    || !source[first.span.start].is_uppercase()
                    || !source[second.span.start].is_uppercase()
                    || first.span.len() < 3
                    || second.span.len() < 3
                {
                    continue;
                }
                let text_of = |token: &Token| -> String {
                    document.get_span_content(&token.span).iter().collect()
                };
                let (first_text, second_text) = (text_of(first), text_of(second));
                // *Sonntag Nachmittag*, *Mai Abend*: a day and a time are two words.
                if TIME_WORDS.contains(&first_text.as_str()) {
                    continue;
                }
                // *Der Katze Fell*: an article that fits the first noun and not the second belongs
                // to the first, and the second is an object of its own.
                if at > 0
                    && let Some(readings) = determiner_readings(&text_of(tokens[at - 1]))
                {
                    let fits = |genders: GenderSet| {
                        readings.iter().any(|r| {
                            r.gender
                                .is_some_and(|g| genders.contains(GenderSet::from(g)))
                        })
                    };
                    let (first_fits, second_fits) = (
                        fits(self.nouns.genders(&first_text)),
                        fits(self.nouns.genders(&second_text)),
                    );
                    if first_fits && !second_fits {
                        continue;
                    }
                }
                let a: Vec<char> = source[first.span.start..first.span.end].to_vec();
                let b: Vec<char> = source[second.span.start..second.span.end]
                    .iter()
                    .flat_map(|c| c.to_lowercase())
                    .collect();
                // All capitals (*USB Kabel*) are an abbreviation, not a noun in this sense.
                if a.iter().all(|c| !c.is_lowercase()) {
                    continue;
                }
                let joined = ["", "s", "n", "en"].iter().find_map(|link| {
                    let candidate: Vec<char> = a
                        .iter()
                        .copied()
                        .chain(link.chars())
                        .chain(b.iter().copied())
                        .collect();
                    self.entries
                        .get_word_metadata(&candidate)
                        // A name (*Hanspeter*) is how some write two names, not a compound.
                        .filter(|meta| {
                            meta.is_noun()
                                && !meta
                                    .noun
                                    .as_ref()
                                    .is_some_and(|n| n.is_proper == Some(true))
                        })
                        .map(|_| candidate)
                });
                if let Some(joined) = joined {
                    lints.push(Lint {
                        span: harper_core::Span::new(first.span.start, second.span.end),
                        lint_kind: LintKind::BoundaryError,
                        suggestions: vec![Suggestion::ReplaceWith(joined.clone())],
                        priority: 33,
                        message: format!(
                            "Zusammengesetzte Nomen schreibt man zusammen: »{}«.",
                            joined.iter().collect::<String>()
                        ),
                    });
                }
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Schreibt zwei Nomen zusammen, wenn das Wörterbuch das Kompositum kennt."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanSplitCompound;
    use crate::parsers::PlainGerman;
    use crate::spell::{base_german_dictionary, compound_aware_german_dictionary};
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn count(text: &str) -> usize {
        let dict = compound_aware_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanSplitCompound::new(base_german_dictionary())
            .lint(&document)
            .len()
    }

    #[test]
    fn two_nouns_that_make_a_compound_are_written_together() {
        for text in [
            "Die Kaffee Maschine ist kaputt.",
            "Wir haben einen Computer Bildschirm gekauft.",
            "Das ist ein Haus Schlüssel.",
        ] {
            assert_eq!(count(text), 1, "{text}");
        }
    }

    #[test]
    fn other_neighbours_are_left_alone() {
        for text in [
            "Die Kaffeemaschine ist kaputt.",
            "Er gibt dem Kind Geld.",
            "Herr Müller kommt heute.",
            "Peter Schmidt wohnt hier.",
            "Das Neue Rathaus Berlin Mitte Ist Offen.",
            "Wir kaufen Papier Stifte.",
            "Ich bin froh, dass Hans Peter Doskozil das macht.",
        ] {
            assert_eq!(count(text), 0, "{text}");
        }
    }
}
