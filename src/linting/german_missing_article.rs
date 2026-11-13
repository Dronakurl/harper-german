//! *Die Kinder haben in Garten gespielt*, *Er geht zu Schule*, *Sie ist bei Arzt*: a place
//! behind a preposition without its article.

use harper_core::{
    Document, Punctuation, Span, Token, TokenKind, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
};

use crate::grammar::noun_gender::NounGender;
use crate::grammar::noun_phrase::lowercase_of;
use crate::morphology::Gender;

/// Places and institutions that take an article behind a preposition: *im Garten*, *zur
/// Schule*, *beim Arzt*. Each is a singular a writer means as one particular place.
const PLACES: &[&str] = &[
    "Garten",
    "Schule",
    "Arzt",
    "Zahnarzt",
    "Kino",
    "Bahnhof",
    "Supermarkt",
    "Park",
    "Büro",
    "Bäcker",
    "Friseur",
    "Kirche",
    "Uni",
    "Universität",
    "Bank",
    "Post",
    "Apotheke",
    "Krankenhaus",
    "Theater",
    "Strand",
    "Flughafen",
    "Markt",
    "Restaurant",
    "Hotel",
    "Museum",
    "Zoo",
    "Schwimmbad",
    "Kindergarten",
    "Spielplatz",
    "Metzger",
    "Bibliothek",
    "Rathaus",
    "Polizei",
    "Hausarzt",
    "Stadion",
    "Fitnessstudio",
    "Arbeit",
];

/// Reports a preposition and a place without the article it needs, when the place ends the
/// phrase: *in Garten* → *im Garten*, *zu Schule* → *zur Schule*, *bei Arzt* → *beim Arzt*,
/// *von Arbeit* → *von der Arbeit*. *in Arbeit* (being worked on) and *zu Arbeit* are left
/// alone.
#[derive(Default)]
pub struct GermanMissingArticle {
    nouns: NounGender,
}

impl GermanMissingArticle {
    fn fixed(preposition: &str, gender: Gender) -> Option<Vec<&'static str>> {
        let feminine = gender == Gender::Feminine;
        Some(match (preposition, feminine) {
            ("in", false) if gender == Gender::Masculine => vec!["im", "in den"],
            ("in", false) => vec!["im", "ins"],
            ("in", true) => vec!["in der", "in die"],
            ("zu", false) => vec!["zum"],
            ("zu", true) => vec!["zur"],
            ("bei", false) => vec!["beim"],
            ("bei", true) => vec!["bei der"],
            ("von", false) => vec!["vom"],
            ("von", true) => vec!["von der"],
            ("aus", false) => vec!["aus dem"],
            ("aus", true) => vec!["aus der"],
            _ => return None,
        })
    }
}

impl Linter for GermanMissingArticle {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            for at in 0..words.len().saturating_sub(1) {
                let preposition = lowercase_of(words[at], document);
                let noun = words[at + 1];
                let place = document.get_span_content_str(&noun.span);
                if !PLACES.contains(&place.as_str()) {
                    continue;
                }
                if place == "Arbeit" && !matches!(preposition.as_str(), "von" | "bei") {
                    continue;
                }
                // *von Schule zu Schule*: the place twice, without articles.
                let same = |at: usize| {
                    words
                        .get(at)
                        .is_some_and(|t| document.get_span_content_str(&t.span) == place)
                };
                if (at >= 2 && same(at - 1)) || same(at + 3) {
                    continue;
                }
                // The phrase ends at the place: a punctuation mark or a lower-case word
                // behind it, not *in Garten und Haus* or *zu Schule gehen*-like lists of
                // nouns, nor a compound with a hyphen.
                let ends = words.get(at + 2).is_none_or(|next| {
                    matches!(
                        next.kind,
                        TokenKind::Punctuation(
                            Punctuation::Period | Punctuation::Bang | Punctuation::Question
                        )
                    ) || (next.kind.is_word()
                        && document
                            .get_span_content(&next.span)
                            .first()
                            .is_some_and(|c| c.is_lowercase())
                        && !matches!(
                            lowercase_of(next, document).as_str(),
                            "und" | "oder" | "bzw" | "sowie"
                        ))
                });
                if !ends || document.get_full_content().get(noun.span.end) == Some(&'-') {
                    continue;
                }
                let Some((gender, _)) = self.nouns.gender_of(&place) else {
                    continue;
                };
                let Some(fixes) = Self::fixed(&preposition, gender) else {
                    continue;
                };
                let written = document.get_span_content_str(&words[at].span);
                let capital = written.chars().next().is_some_and(char::is_uppercase);
                let suggestions = fixes
                    .iter()
                    .map(|fix| {
                        let mut fix = fix.to_string();
                        if capital {
                            fix = fix[..1].to_uppercase() + &fix[1..];
                        }
                        Suggestion::ReplaceWith(format!("{fix} {place}").chars().collect())
                    })
                    .collect();
                lints.push(Lint {
                    span: Span::new(words[at].span.start, noun.span.end),
                    lint_kind: LintKind::Grammar,
                    suggestions,
                    message: format!("Vor »{place}« fehlt hier der Artikel."),
                    priority: 31,
                });
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Ergänzt den Artikel bei Orten nach einer Präposition (»im Garten«, »zur Schule«)."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanMissingArticle;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn fixes(text: &str) -> Vec<String> {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanMissingArticle::default()
            .lint(&document)
            .into_iter()
            .flat_map(|lint| lint.suggestions)
            .map(|suggestion| suggestion.to_string())
            .collect()
    }

    #[test]
    fn a_place_without_its_article_is_reported() {
        assert_eq!(
            fixes("Die Kinder haben in Garten gespielt."),
            ["Replace with: “im Garten”", "Replace with: “in den Garten”"]
        );
        assert_eq!(fixes("Er geht zu Schule."), ["Replace with: “zur Schule”"]);
        assert_eq!(fixes("Sie ist bei Arzt."), ["Replace with: “beim Arzt”"]);
        assert_eq!(
            fixes("Ich komme von Arbeit."),
            ["Replace with: “von der Arbeit”"]
        );
    }

    #[test]
    fn places_with_an_article_or_idioms_are_quiet() {
        for text in [
            "Die Kinder haben im Garten gespielt.",
            "Er geht zur Schule.",
            "Das Projekt ist in Arbeit.",
            "Er ist zu Hause.",
            "Wir gehen in Garten und Park spazieren.",
            "Sie arbeitet in Schule-Projekten.",
            "Er kommt von Schule zu Schule.",
            "Die Vereine von Kindergarten, Schule und Kirche helfen.",
        ] {
            assert!(fixes(text).is_empty(), "{text}: {:?}", fixes(text));
        }
    }
}
