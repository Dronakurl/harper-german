//! Weak masculine nouns without their ending: *mit dem Präsident*, *seinem
//! Kollege*, *den Student*, *einen Nachbar* (n-Deklination).

use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Token, TokenKind, TokenStringExt};

use crate::grammar::noun_phrase::lowercase_of;

/// Weak masculine nouns and their form in every case but the nominative
/// singular. A closed list: the class is recognizable, but a noun that only
/// looks like one (*der Patent*? no: *das Patent*) would be a false alarm.
const WEAK_NOUNS: &[(&str, &str)] = &[
    ("Präsident", "Präsidenten"),
    ("Student", "Studenten"),
    ("Patient", "Patienten"),
    ("Assistent", "Assistenten"),
    ("Dozent", "Dozenten"),
    ("Konkurrent", "Konkurrenten"),
    ("Praktikant", "Praktikanten"),
    ("Elefant", "Elefanten"),
    ("Lieferant", "Lieferanten"),
    ("Polizist", "Polizisten"),
    ("Journalist", "Journalisten"),
    ("Tourist", "Touristen"),
    ("Spezialist", "Spezialisten"),
    ("Kandidat", "Kandidaten"),
    ("Soldat", "Soldaten"),
    ("Automat", "Automaten"),
    ("Fotograf", "Fotografen"),
    ("Biologe", "Biologen"),
    ("Psychologe", "Psychologen"),
    ("Kollege", "Kollegen"),
    ("Kunde", "Kunden"),
    ("Junge", "Jungen"),
    ("Experte", "Experten"),
    ("Zeuge", "Zeugen"),
    ("Neffe", "Neffen"),
    ("Löwe", "Löwen"),
    ("Affe", "Affen"),
    ("Hase", "Hasen"),
    ("Franzose", "Franzosen"),
    ("Nachbar", "Nachbarn"),
    ("Bauer", "Bauern"),
    ("Mensch", "Menschen"),
    ("Held", "Helden"),
    ("Herr", "Herrn"),
];

/// Determiners that are dative or accusative only, never a nominative
/// singular masculine: behind them the weak noun takes its ending.
const OBLIQUE: &[&str] = &[
    "dem", "den", "einem", "einen", "keinem", "keinen", "diesem", "diesen", "jedem", "jeden",
    "meinem", "meinen", "deinem", "deinen", "seinem", "seinen", "ihrem", "ihren", "unserem",
    "unseren", "eurem", "euren", "welchem", "welchen",
];

#[derive(Default)]
pub struct GermanWeakNoun;

impl Linter for GermanWeakNoun {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|t| !t.kind.is_whitespace())
                .collect();
            for (at, word) in words.iter().enumerate() {
                let written = document.get_span_content_str(&word.span);
                // *des Nachbars*, *des Präsident*: the genitive is weak as well.
                if at > 0
                    && lowercase_of(words[at - 1], document) == "des"
                    && let Some((_, form)) = WEAK_NOUNS
                        .iter()
                        .find(|(base, _)| *base == written || format!("{base}s") == written)
                    && !document
                        .get_full_content()
                        .get(word.span.end)
                        .is_some_and(|c| *c == '-')
                    && *form != written
                {
                    lints.push(Lint {
                        span: word.span,
                        lint_kind: LintKind::Agreement,
                        suggestions: vec![Suggestion::ReplaceWith(form.chars().collect())],
                        priority: 31,
                        message: format!(
                            "»{written}« wird schwach gebeugt: nach »des« heißt es »{form}«."
                        ),
                    });
                    continue;
                }
                let Some((_, oblique_form)) = WEAK_NOUNS.iter().find(|(base, _)| *base == written)
                else {
                    continue;
                };
                // A hyphen behind it makes it part of a compound
                // (*Präsident-Wahl*).
                if document.get_full_content().get(word.span.end) == Some(&'-') {
                    continue;
                }
                // The determiner in front, with at most two adjectives between:
                // *dem neuen Präsident*.
                let determiner = (1..=3).find_map(|back| {
                    let t = words.get(at.checked_sub(back)?)?;
                    let lower = lowercase_of(t, document);
                    // *den Preis, den Herr Holtz nannte*: behind a comma it is a
                    // relative pronoun, the object of its clause.
                    let behind_comma = at
                        .checked_sub(back + 1)
                        .and_then(|i| words.get(i))
                        .is_some_and(|p| matches!(p.kind, TokenKind::Punctuation(_)));
                    if OBLIQUE.contains(&lower.as_str()) && behind_comma {
                        Some(String::new())
                    } else if OBLIQUE.contains(&lower.as_str()) {
                        Some(lower)
                    } else if back < 3
                        && matches!(t.kind, TokenKind::Word(_))
                        && lower.ends_with("en")
                        && document
                            .get_span_content(&t.span)
                            .first()
                            .is_some_and(|c| c.is_lowercase())
                    {
                        None
                    } else {
                        Some(String::new())
                    }
                });
                let Some(determiner) = determiner.filter(|d| !d.is_empty()) else {
                    continue;
                };
                lints.push(Lint {
                    span: word.span,
                    lint_kind: LintKind::Agreement,
                    suggestions: vec![Suggestion::ReplaceWith(oblique_form.chars().collect())],
                    priority: 31,
                    message: format!(
                        "»{written}« wird schwach gebeugt: nach »{determiner}« heißt es »{oblique_form}«."
                    ),
                });
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Prüft die Endung schwach gebeugter Nomen: »mit dem Präsidenten«, »seinem Kollegen«."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanWeakNoun;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn reported(text: &str) -> Vec<String> {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanWeakNoun
            .lint(&document)
            .iter()
            .map(|l| document.get_span_content_str(&l.span))
            .collect()
    }

    #[test]
    fn a_weak_noun_without_its_ending_is_reported() {
        assert_eq!(reported("Wir sprachen mit dem Präsident."), ["Präsident"]);
        assert_eq!(
            reported("Er hilft seinem Kollege bei der Arbeit."),
            ["Kollege"]
        );
        assert_eq!(reported("Sie traf den Student in der Mensa."), ["Student"]);
        assert_eq!(reported("Wir haben einen neuen Kunde."), ["Kunde"]);
        assert_eq!(reported("Der Hund des Nachbars bellt."), ["Nachbars"]);
        assert_eq!(reported("Das Haus des Präsident ist groß."), ["Präsident"]);
        assert_eq!(reported("Das Auto des Kunde steht da."), ["Kunde"]);
    }

    #[test]
    fn the_nominative_and_the_right_ending_are_quiet() {
        for text in [
            "Der Präsident kam.",
            "Wir sprachen mit dem Präsidenten.",
            "Ein Kunde wartet.",
            "Das ist mein Kollege.",
            "Die Präsident-Wahl beginnt.",
            "Sie gab den Kindern den Ball, der Junge lachte.",
            "Ich zahlte den Preis, den Herr Holtz genannt hatte.",
            "Der Hund des Nachbarn bellt.",
            "Das Haus des Präsidenten ist groß.",
        ] {
            assert!(reported(text).is_empty(), "{text}: {:?}", reported(text));
        }
    }
}
