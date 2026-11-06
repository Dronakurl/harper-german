use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Punctuation, Token, TokenKind, TokenStringExt};

/// Verbs and nouns where only the prefix `wider-` ("against") is correct.
///
/// Matching is by prefix of the remainder rather than by whole word, so one
/// entry covers the whole paradigm: `sprech` catches "widersprechen",
/// "widerspricht" and "widersprechende".
const WIDER_ONLY: &[&str] = &[
    "fahr", "fuhr", "hall", "leg", "ruf", "setz", "sinn", "spieg", "sproch", "sprech", "sprich",
    "spruch", "stand", "steh", "streb", "willen",
];

/// Verbs and nouns where only the prefix `wieder-` ("again") is correct.
const WIEDER_ONLY: &[&str] = &[
    "aufbau", "aufnahm", "beleb", "entdeck", "eröffn", "erkann", "erkenn", "gutmach", "herstell",
    "hol", "sah", "seh", "sieh", "vereinig", "verwend", "wahl", "wähl",
];

/// Catches the `wider-` / `wieder-` prefix mix-up.
///
/// The two are unrelated — `wider` means "against", `wieder` means "again" —
/// but they differ by one letter and are among the most commonly confused
/// prefixes in written German. The spell checker cannot catch either direction
/// on its own: the compound splitter happily reads "wiederspiegeln" as
/// `wieder` + `spiegeln`, both of which are real words.
///
/// Only the two closed lists above are consulted, so genuinely ambiguous stems
/// ("wiederholen" the verb vs. "widerhallen") are never guessed at.
#[derive(Default)]
pub struct GermanWiderWieder;

impl GermanWiderWieder {
    /// The corrected spelling of `word`, if it carries the wrong prefix.
    fn correction(word: &[char]) -> Option<String> {
        let lower: String = word
            .iter()
            .flat_map(|c| c.to_lowercase())
            .collect::<String>();

        // Check `wieder` first: it is the longer prefix, and every `wieder`
        // also starts with `wied`, never with `wider`.
        let (right, rest, stems) = match lower.strip_prefix("wieder") {
            Some(rest) => ("wider", rest, WIDER_ONLY),
            None => ("wieder", lower.strip_prefix("wider")?, WIEDER_ONLY),
        };

        if rest.is_empty() || !stems.iter().any(|stem| rest.starts_with(stem)) {
            return None;
        }

        let mut fixed = String::with_capacity(lower.len());
        fixed.push_str(right);
        fixed.push_str(rest);

        // Preserve a leading capital: the word may open a sentence or be a
        // nominalisation ("Widerspruch", "Wiederholung").
        if word.first().is_some_and(|c| c.is_uppercase()) {
            let mut chars = fixed.chars();
            if let Some(first) = chars.next() {
                fixed = first.to_uppercase().chain(chars).collect();
            }
        }

        Some(fixed)
    }
}

impl GermanWiderWieder {
    /// *Das spiegelt sich in den Zahlen wieder*: the separated particle of
    /// *widerspiegeln* and *widerhallen*, the two separable verbs of the
    /// family, at the end of the clause. *widersprechen*, *widerfahren* and
    /// *widerrufen* are never separated, so *er spricht das wieder* is "again".
    fn separated_particles(document: &Document) -> Vec<Lint> {
        const STEMS: &[&str] = &["spiegel", "hall"];
        let lower = |t: &Token| -> String {
            document
                .get_span_content(&t.span)
                .iter()
                .flat_map(|c| c.to_lowercase())
                .collect()
        };
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|t| !t.kind.is_whitespace())
                .collect();
            for (at, word) in words.iter().enumerate() {
                let finite = lower(word);
                // A finite form: *spiegelt*, *spiegeln*, *spiegelte*, *hallte*.
                let Some(stem) = STEMS.iter().find(|stem| finite.starts_with(**stem)) else {
                    continue;
                };
                let ending = &finite[stem.len()..];
                if !["t", "n", "te", "ten", "st", "test", "e"].contains(&ending) {
                    continue;
                }
                // The clause runs to the next punctuation; *wieder* ends it.
                let Some(end) = words[at + 1..]
                    .iter()
                    .position(|t| matches!(t.kind, TokenKind::Punctuation(_)))
                    .map(|i| at + 1 + i)
                else {
                    continue;
                };
                if end == at + 1 || lower(words[end - 1]) != "wieder" {
                    continue;
                }
                // *spiegeln* takes *sich* in this sense; *hallen* does not need it.
                if *stem == "spiegel" && !words[at + 1..end].iter().any(|t| lower(t) == "sich") {
                    continue;
                }
                if !matches!(
                    words[end].kind,
                    TokenKind::Punctuation(
                        Punctuation::Period
                            | Punctuation::Comma
                            | Punctuation::Bang
                            | Punctuation::Question
                            | Punctuation::Semicolon
                    )
                ) {
                    continue;
                }
                lints.push(Lint {
                    span: words[end - 1].span,
                    lint_kind: LintKind::Spelling,
                    suggestions: vec![Suggestion::ReplaceWith("wider".chars().collect())],
                    message: format!(
                        "»{finite} … wider« gehört zu »wider{stem}n« (»wider« heißt »gegen«), nicht zu »wieder« (noch einmal)."
                    ),
                    priority: 19,
                });
            }
        }
        lints
    }
}

impl Linter for GermanWiderWieder {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Self::separated_particles(document);

        for word in document.iter_words() {
            let chars = document.get_span_content(&word.span);
            let Some(fixed) = Self::correction(chars) else {
                continue;
            };

            let original: String = chars.iter().collect();
            lints.push(Lint {
                span: word.span,
                lint_kind: LintKind::Spelling,
                suggestions: vec![Suggestion::ReplaceWith(fixed.chars().collect())],
                message: format!(
                    "»{original}« verwechselt die Vorsilben: »wider« heißt »gegen«, \
                     »wieder« heißt »noch einmal«. Gemeint ist »{fixed}«."
                ),
                priority: 19,
            });
        }

        lints
    }

    fn description(&self) -> &str {
        "Unterscheidet die Vorsilben »wider« (gegen) und »wieder« (noch einmal)."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanWiderWieder;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn count(text: &str) -> usize {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanWiderWieder.lint(&document).len()
    }

    #[test]
    fn a_separated_wider_is_reported() {
        assert_eq!(count("Das spiegelt sich in den Zahlen wieder."), 1);
        assert_eq!(
            count("Die Ergebnisse spiegelten sich deutlich wieder, sagte er."),
            1
        );
        assert_eq!(count("Der Ruf hallte im Tal wieder."), 1);
        for text in [
            "Das spiegelt sich in den Zahlen wider.",
            "Er spricht das Thema wieder an.",
            "Er spricht das wieder.",
            "Sie spiegelt den Ball wieder.",
            "Er ruft morgen wieder.",
            "Das Ergebnis spiegelt sich wieder in den Zahlen.",
        ] {
            assert_eq!(count(text), 0, "{text}");
        }
    }

    fn correction(word: &str) -> Option<String> {
        let chars: Vec<char> = word.chars().collect();
        GermanWiderWieder::correction(&chars)
    }

    #[test]
    fn corrects_wieder_to_wider() {
        for (wrong, right) in [
            ("wiederspiegeln", "widerspiegeln"),
            ("wiederspiegelt", "widerspiegelt"),
            ("wiedersprechen", "widersprechen"),
            ("wiederspricht", "widerspricht"),
            ("Wiederspruch", "Widerspruch"),
            ("wiederstehen", "widerstehen"),
            ("Wiederstand", "Widerstand"),
            ("wiederrufen", "widerrufen"),
            ("wiederlegen", "widerlegen"),
        ] {
            assert_eq!(correction(wrong).as_deref(), Some(right), "{wrong}");
        }
    }

    #[test]
    fn corrects_wider_to_wieder() {
        for (wrong, right) in [
            ("widerholen", "wiederholen"),
            ("Widerholung", "Wiederholung"),
            ("widersehen", "wiedersehen"),
            ("Widersehen", "Wiedersehen"),
            ("widerherstellen", "wiederherstellen"),
            ("Widervereinigung", "Wiedervereinigung"),
            ("widerbeleben", "wiederbeleben"),
            ("Widerwahl", "Wiederwahl"),
        ] {
            assert_eq!(correction(wrong).as_deref(), Some(right), "{wrong}");
        }
    }

    /// Both prefixes are real words with real paradigms; only the closed stem
    /// lists may be corrected.
    #[test]
    fn leaves_correct_and_ambiguous_words_alone() {
        for word in [
            "wider",
            "wieder",
            "wiederum",
            "widerlich",
            "widerwärtig",
            "Widerspruch",
            "widersprechen",
            "widerspiegeln",
            "Widerstand",
            "wiederholen",
            "Wiederholung",
            "Wiedersehen",
            "wiederkommen",
            "Wiederaufbau",
            "Widerrede",
            "Kleider",
            "Glieder",
        ] {
            assert_eq!(correction(word), None, "{word} should be left alone");
        }
    }
}
