//! *Ich habe 25 Jahre alt*, *Ich bin Hunger*, *Er ist das Buch gelesen*, *Ich habe kalt*: the
//! wrong one of *sein* and *haben*.

use harper_core::{
    Document, Punctuation, Span, Token, TokenKind, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
};

use crate::grammar::noun_phrase::lowercase_of;

const PAIRS: &[(&str, &str)] = &[
    ("habe", "bin"),
    ("hast", "bist"),
    ("hat", "ist"),
    ("haben", "sind"),
    ("habt", "seid"),
];

/// Participles of verbs with an accusative object that form their perfect with *haben*.
const TRANSITIVE_PARTICIPLES: &[&str] = &[
    "gelesen",
    "gekauft",
    "gegessen",
    "getrunken",
    "gesehen",
    "gehört",
    "gemacht",
    "geschrieben",
    "gefunden",
    "genommen",
    "bekommen",
    "gesagt",
    "gebaut",
    "verloren",
    "vergessen",
    "gebracht",
    "gegeben",
    "gekocht",
    "gewonnen",
    "bezahlt",
    "geschickt",
    "gebacken",
];

const SEIN_FORMS: &[(&str, &str)] = &[
    ("bin", "habe"),
    ("bist", "hast"),
    ("ist", "hat"),
    ("sind", "haben"),
    ("seid", "habt"),
    ("war", "hatte"),
    ("waren", "hatten"),
];

const DATIVE: &[(&str, &str)] = &[
    ("ich", "Mir"),
    ("du", "Dir"),
    ("wir", "Uns"),
    ("ihr", "Euch"),
    ("er", "Ihm"),
];

/// Reports
///
/// * *haben* with *25 Jahre alt*: → *bin*;
/// * *sein* with *Hunger*, *Durst*, *Angst*, *Lust* behind *ich*, *du*, *wir*, *ihr*: → *habe*;
/// * *sein* with an accusative object and a participle that takes *haben*: *Er ist das Buch
///   gelesen* → *hat*;
/// * *haben* with *kalt*, *warm*, *heiß*: *Ich habe kalt* → *Mir ist kalt*.
#[derive(Default)]
pub struct GermanSeinHaben;

impl GermanSeinHaben {
    fn replace(token: &Token, with: &str, message: &str) -> Lint {
        Lint {
            span: token.span,
            lint_kind: LintKind::Grammar,
            suggestions: vec![Suggestion::ReplaceWith(with.chars().collect())],
            message: message.to_owned(),
            priority: 31,
        }
    }
}

impl Linter for GermanSeinHaben {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            let lower: Vec<String> = words.iter().map(|t| lowercase_of(t, document)).collect();
            let capital = |at: usize| {
                document
                    .get_span_content(&words[at].span)
                    .first()
                    .is_some_and(|c| c.is_uppercase())
            };
            for at in 0..words.len() {
                // *habe 25 Jahre alt*
                if let Some((_, sein)) = PAIRS.iter().find(|(h, _)| *h == lower[at])
                    && matches!(
                        words.get(at + 1).map(|t| &t.kind),
                        Some(TokenKind::Number(_))
                    )
                    && lower
                        .get(at + 2)
                        .is_some_and(|w| w == "jahre" || w == "jahren")
                    && lower.get(at + 3).is_some_and(|w| w == "alt")
                {
                    let sein = if capital(at) { "Bin" } else { sein };
                    lints.push(Self::replace(
                        words[at],
                        sein,
                        "Das Alter gibt man mit »sein« an: »ich bin 25 Jahre alt«.",
                    ));
                }
                // *Ich bin Hunger*
                if at > 0
                    && let Some((_, haben)) = SEIN_FORMS.iter().find(|(s, _)| *s == lower[at])
                    && matches!(lower[at - 1].as_str(), "ich" | "du" | "wir" | "ihr")
                    && words.get(at + 1).is_some_and(|_| capital(at + 1))
                    && matches!(
                        lower.get(at + 1).map(String::as_str),
                        Some("hunger" | "durst" | "angst" | "lust")
                    )
                {
                    lints.push(Self::replace(
                        words[at],
                        haben,
                        "Mit »Hunger«, »Durst«, »Angst« und »Lust« steht »haben«.",
                    ));
                }
                // *Er ist das Buch gelesen*
                if at > 0
                    && let Some((_, haben)) = SEIN_FORMS.iter().find(|(s, _)| *s == lower[at])
                    && matches!(
                        lower[at - 1].as_str(),
                        "ich" | "du" | "er" | "sie" | "es" | "wir" | "ihr"
                    )
                    && matches!(
                        lower.get(at + 1).map(String::as_str),
                        Some("das" | "den" | "die" | "ein" | "einen" | "eine")
                    )
                    && words
                        .get(at + 2)
                        .is_some_and(|t| t.kind.is_noun() && capital(at + 2))
                    && lower
                        .get(at + 3)
                        .is_some_and(|w| TRANSITIVE_PARTICIPLES.contains(&w.as_str()))
                    && words.get(at + 4).is_none_or(|t| {
                        matches!(
                            t.kind,
                            TokenKind::Punctuation(
                                Punctuation::Period | Punctuation::Bang | Punctuation::Comma
                            )
                        )
                    })
                {
                    lints.push(Self::replace(
                        words[at],
                        haben,
                        "Ein Verb mit Akkusativobjekt bildet das Perfekt mit »haben«.",
                    ));
                }
                // *Ich habe kalt*: only a sentence of three words, subject first.
                if at == 1
                    && PAIRS.iter().any(|(h, _)| *h == lower[at])
                    && let Some((_, dative)) = DATIVE.iter().find(|(p, _)| *p == lower[0])
                    && lower
                        .get(2)
                        .is_some_and(|w| matches!(w.as_str(), "kalt" | "warm" | "heiß"))
                    && words
                        .get(3)
                        .is_none_or(|t| matches!(t.kind, TokenKind::Punctuation(_)))
                {
                    lints.push(Lint {
                        span: Span::new(words[0].span.start, words[2].span.end),
                        lint_kind: LintKind::Grammar,
                        suggestions: vec![Suggestion::ReplaceWith(
                            format!("{dative} ist {}", lower[2]).chars().collect(),
                        )],
                        message: "Man sagt »mir ist kalt«, nicht »ich habe kalt«.".to_owned(),
                        priority: 31,
                    });
                }
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Prüft »sein« und »haben« bei Alter, Hunger, Perfekt mit Objekt und Kälte."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanSeinHaben;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn fixes(text: &str) -> Vec<String> {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanSeinHaben
            .lint(&document)
            .into_iter()
            .flat_map(|l| l.suggestions)
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn the_wrong_auxiliary_is_reported() {
        for (text, fixed) in [
            ("Ich habe 25 Jahre alt.", "bin"),
            ("Er hat 40 Jahre alt.", "ist"),
            ("Ich bin Hunger.", "habe"),
            ("Wir sind Durst.", "haben"),
            ("Er ist das Buch gelesen.", "hat"),
            ("Ich bin einen Kuchen gegessen.", "habe"),
            ("Ich habe kalt.", "Mir ist kalt"),
        ] {
            assert_eq!(fixes(text), [format!("Replace with: “{fixed}”")], "{text}");
        }
    }

    #[test]
    fn the_right_forms_are_quiet() {
        for text in [
            "Ich bin 25 Jahre alt.",
            "Ich habe Hunger.",
            "Er hat das Buch gelesen.",
            "Das Buch ist gelesen.",
            "Er ist den Weg gegangen.",
            "Mir ist kalt.",
            "Ich habe 25 Euro.",
            "Das ist Angst.",
            "Er ist das Gesicht gewesen.",
        ] {
            assert!(fixes(text).is_empty(), "{text}: {:?}", fixes(text));
        }
    }
}
