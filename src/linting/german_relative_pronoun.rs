//! A relative pronoun that does not fit its noun: *Das Mädchen, **der** dort
//! spielt*, *Die Leute, **wo** ich kenne*.

use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Punctuation, Token, TokenKind, TokenStringExt};

use crate::grammar::determiners::determiner_readings;
use crate::grammar::noun_gender::NounGender;
use crate::grammar::noun_phrase::lowercase_of;
use crate::linting::german_relative_clause_comma::GermanRelativeClauseComma;
use crate::morphology::{Gender, GenderSet};
use crate::spell::curated_german_dictionary;
use harper_core::spell::Dictionary;

const RELATIVES: &[&str] = &["der", "die", "das", "dem", "den", "dessen", "deren"];

/// Subjects that may open the relative clause: then the pronoun is an object,
/// and its case is not known.
const SUBJECTS: &[&str] = &["ich", "du", "er", "sie", "es", "wir", "ihr", "man"];

/// Nouns for people and things, after which the relative *wo* is always
/// wrong. Places and times (*die Stadt, wo …*, *der Tag, wo …*) take it in
/// speech and are left alone.
const NOT_PLACES: &[&str] = &[
    "Leute", "Menschen", "Mensch", "Mann", "Männer", "Frau", "Frauen", "Kind", "Kinder", "Freund",
    "Freunde", "Freundin", "Person", "Personen", "Kollege", "Kollegen", "Kollegin", "Typ", "Typen",
    "Lehrer", "Lehrerin", "Nachbar", "Nachbarn", "Buch", "Bücher", "Auto", "Autos", "Film",
    "Filme", "Sache", "Sachen", "Ding", "Dinge",
];

/// Reports a relative pronoun whose gender cannot be the noun's.
///
/// The pattern needs no parser where it is narrow: an article and a
/// singular noun, a comma, *der/die/das/…*, then a lower-case word and a verb
/// that ends the clause (the same test the comma rule uses). A capitalized
/// word behind the pronoun makes it an article (*Das Mädchen, der Hund und die
/// Katze*), and a plural or an unknown gender is never checked.
#[derive(Default)]
pub struct GermanRelativePronoun {
    nouns: NounGender,
}

fn nominative(gender: Gender) -> &'static str {
    match gender {
        Gender::Masculine => "der",
        Gender::Feminine => "die",
        Gender::Neuter => "das",
    }
}

impl GermanRelativePronoun {
    fn check(&self, words: &[&Token], comma: usize, document: &Document) -> Option<Lint> {
        let noun = words.get(comma.checked_sub(1)?)?;
        let article = words.get(comma.checked_sub(2)?)?;
        let pronoun = words.get(comma + 1)?;
        let next = words.get(comma + 2)?;
        let noun_text = document.get_span_content_str(&noun.span);
        // *dessen* and *deren* stand before the noun they own: *die Frau, deren Mann*.
        let genitive = matches!(
            document.get_span_content_str(&pronoun.span).as_str(),
            "dessen" | "deren"
        );
        if !noun_text.chars().next().is_some_and(char::is_uppercase)
            || !matches!(next.kind, TokenKind::Word(_))
            || (!genitive
                && (next.kind.is_noun()
                    && !SUBJECTS.contains(&lowercase_of(next, document).as_str())
                    || document
                        .get_span_content(&next.span)
                        .first()
                        .is_some_and(|c| c.is_uppercase())))
        {
            return None;
        }
        let pronoun_text = document.get_span_content_str(&pronoun.span);
        let next_lower = lowercase_of(next, document);

        // *Die Leute, wo ich kenne*.
        if pronoun_text == "wo" && NOT_PLACES.contains(&noun_text.as_str()) {
            GermanRelativeClauseComma::clause_end(words, comma + 1, document)?;
            let plural = determiner_readings(&lowercase_of(article, document))
                .is_some_and(|readings| readings.iter().all(|r| r.gender.is_none()));
            return Some(Lint {
                span: pronoun.span,
                lint_kind: LintKind::Grammar,
                suggestions: if plural {
                    vec![Suggestion::ReplaceWith("die".chars().collect())]
                } else {
                    Vec::new()
                },
                priority: 31,
                message: format!(
                    "Nach »{noun_text}« leitet ein Relativpronomen (»der«, »die«, »das« …) den Satz ein, nicht »wo«."
                ),
            });
        }

        // *das Auto, was ich kaufen will*: after a noun the relative pronoun is *das*, not
        // *was*. Only for a neuter noun, whose *das* serves both cases, and not after a
        // nominalized adjective, which takes *was* (*das Beste, was*).
        if pronoun_text == "was" {
            let lower: String = noun_text.to_lowercase();
            let nominalized = curated_german_dictionary()
                .get_word_metadata(&lower.chars().collect::<Vec<_>>())
                .is_some_and(|m| m.is_adjective());
            let neuter = self
                .nouns
                .gender_of(&noun_text)
                .is_some_and(|(gender, singular)| gender == Gender::Neuter && singular);
            // *gibt dem Kind, was es wünscht*: behind a dative the *was* clause is a free
            // relative, an object of its own.
            let article_is_singular = determiner_readings(&lowercase_of(article, document))
                .is_some_and(|readings| {
                    readings.iter().any(|r| r.gender == Some(Gender::Neuter))
                        && readings.iter().all(|r| {
                            matches!(
                                r.case,
                                crate::morphology::Case::Nominative
                                    | crate::morphology::Case::Accusative
                            )
                        })
                });
            if neuter && !nominalized && article_is_singular {
                GermanRelativeClauseComma::clause_end(words, comma + 1, document)?;
                return Some(Lint {
                    span: pronoun.span,
                    lint_kind: LintKind::Grammar,
                    suggestions: vec![Suggestion::ReplaceWith("das".chars().collect())],
                    priority: 31,
                    message: format!(
                        "Nach dem Nomen »{noun_text}« ist das Relativpronomen »das«, nicht »was«."
                    ),
                });
            }
            return None;
        }

        if !RELATIVES.contains(&pronoun_text.as_str()) {
            return None;
        }
        // *Die Kinder, dessen Eltern arbeiten*: a plural antecedent takes *deren*.
        if pronoun_text == "dessen"
            && self.nouns.is_surely_plural(&noun_text)
            && determiner_readings(&lowercase_of(article, document))
                .is_some_and(|readings| readings.iter().any(|r| r.gender.is_none()))
        {
            return Some(Lint {
                span: pronoun.span,
                lint_kind: LintKind::Agreement,
                suggestions: vec![Suggestion::ReplaceWith("deren".chars().collect())],
                priority: 31,
                message: format!("»{noun_text}« ist ein Plural: Das Relativpronomen ist »deren«."),
            });
        }
        let (gender, surely_singular) = self.nouns.gender_of(&noun_text)?;
        // The article settles the number: it has to read as a singular of the
        // noun's gender, and not as a plural unless the noun is surely singular.
        // A preposition fused with the article stands for it: *im Hotel* is *in dem Hotel*.
        let article_lower = lowercase_of(article, document);
        let fused = matches!(
            article_lower.as_str(),
            "im" | "am"
                | "vom"
                | "zum"
                | "beim"
                | "zur"
                | "ins"
                | "ans"
                | "aufs"
                | "durchs"
                | "fürs"
                | "ums"
        );
        // *Orte im Speicher, die …*: the phrase may belong to a noun in front, which the
        // pronoun then refers to. Only checked when no noun precedes it in the clause.
        if fused {
            let article_at = comma - 2;
            let clause_start = words[..article_at]
                .iter()
                .rposition(|t| t.kind.is_punctuation())
                .map_or(0, |at| at + 1);
            if words[clause_start..article_at]
                .iter()
                .enumerate()
                .any(|(i, t)| {
                    (clause_start + i > 0 || i > 0)
                        && t.kind.is_word()
                        && document
                            .get_span_content(&t.span)
                            .first()
                            .is_some_and(|c| c.is_uppercase())
                })
            {
                return None;
            }
        }
        let article_lower = match article_lower.as_str() {
            "im" | "am" | "vom" | "zum" | "beim" => "dem".to_owned(),
            "zur" => "der".to_owned(),
            "ins" | "ans" | "aufs" | "durchs" | "fürs" | "ums" => "das".to_owned(),
            _ => article_lower,
        };
        let readings = determiner_readings(&article_lower)?;
        let singular = readings.iter().any(|r| r.gender == Some(gender));
        let plural = readings.iter().any(|r| r.gender.is_none());
        if !singular || (plural && !surely_singular) {
            return None;
        }
        let (genders, plural_ok) = GermanRelativeClauseComma::antecedents(&pronoun_text)?;
        if genders.intersects(GenderSet::from(gender)) {
            return None;
        }
        // *die Reaktion eines Tieres auf einen Reiz, die …*: the pronoun may
        // refer to a noun further back, which it fits.
        let fits_further_back = words[..comma - 2].iter().any(|word| {
            let text = document.get_span_content_str(&word.span);
            text.chars().next().is_some_and(char::is_uppercase)
                && self
                    .nouns
                    .gender_of(&text)
                    .is_some_and(|(g, surely_singular)| {
                        genders.intersects(GenderSet::from(g)) || (plural_ok && !surely_singular)
                    })
        });
        // *eine Lagekarte und ein Feuerzeug, die …*: two nouns joined make a
        // plural.
        let coordinated = plural_ok
            && words[..comma - 2].iter().rev().take(6).any(|word| {
                matches!(
                    lowercase_of(word, document).as_str(),
                    "und" | "oder" | "sowie" | "wie"
                )
            });
        if fits_further_back || coordinated {
            return None;
        }
        GermanRelativeClauseComma::clause_end(words, comma + 1, document)?;

        // The pronoun is the subject when no other subject opens the clause;
        // only then is its case, the nominative, known.
        let suggestions = if pronoun_text == "dessen" || pronoun_text == "deren" {
            // The genitive: *die Frau, deren Mann*, *der Mann, dessen Frau*.
            let other = if gender == Gender::Feminine {
                "deren"
            } else {
                "dessen"
            };
            vec![Suggestion::ReplaceWith(other.chars().collect())]
        } else if SUBJECTS.contains(&next_lower.as_str()) {
            Vec::new()
        } else {
            vec![Suggestion::ReplaceWith(
                nominative(gender).chars().collect(),
            )]
        };
        Some(Lint {
            span: pronoun.span,
            lint_kind: LintKind::Agreement,
            suggestions,
            priority: 31,
            message: format!(
                "Das Relativpronomen »{pronoun_text}« passt nicht zu »{noun_text}«: Das Nomen ist {}.",
                match gender {
                    Gender::Masculine => "maskulin",
                    Gender::Feminine => "feminin",
                    Gender::Neuter => "neutral",
                }
            ),
        })
    }
}

impl Linter for GermanRelativePronoun {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|t| !t.kind.is_whitespace())
                .collect();
            for (at, token) in words.iter().enumerate() {
                if matches!(token.kind, TokenKind::Punctuation(Punctuation::Comma))
                    && let Some(lint) = self.check(&words, at, document)
                {
                    lints.push(lint);
                }
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Prüft, ob das Relativpronomen zum Nomen passt: »das Mädchen, das …«, »die Leute, die …«."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanRelativePronoun;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn reported(text: &str) -> Vec<String> {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanRelativePronoun::default()
            .lint(&document)
            .iter()
            .map(|l| document.get_span_content_str(&l.span))
            .collect()
    }

    #[test]
    fn was_after_a_neuter_noun_is_das() {
        assert_eq!(reported("Das ist das Auto, was ich kaufen will."), ["was"]);
        assert_eq!(reported("Ich lese ein Buch, was mir gefällt."), ["was"]);
        for text in [
            "Das ist das Beste, was ich je gegessen habe.",
            "Er tat alles, was er konnte.",
            "Es ist das Einzige, was zählt.",
            "Sabine gibt dem Kind, was es wünscht.",
            "Das ist der Grund, warum ich gehe.",
            "Das ist das Auto, das ich kaufen will.",
        ] {
            assert!(reported(text).is_empty(), "{text}: {:?}", reported(text));
        }
    }

    #[test]
    fn a_pronoun_behind_a_fused_article_is_checked() {
        assert_eq!(
            reported("Wir wohnen im Hotel, der sehr teuer ist."),
            ["der"]
        );
        assert!(reported("Wir wohnen im Hotel, das sehr teuer ist.").is_empty());
        assert!(reported("Er kam zur Tür, die offen stand.").is_empty());
        assert!(reported("Variablen sind Orte im Speicher, die einen Wert aufnehmen.").is_empty());
    }

    #[test]
    fn a_pronoun_of_the_wrong_gender_is_reported() {
        assert_eq!(
            reported("Das Mädchen, der dort spielt, ist meine Tochter."),
            ["der"]
        );
        assert_eq!(reported("Ich kenne den Mann, die dort wohnt."), ["die"]);
        assert_eq!(reported("Die Leute, wo ich kenne, sind nett."), ["wo"]);
        assert_eq!(
            reported("Die Frau, dessen Mann krank ist, kommt nicht."),
            ["dessen"]
        );
        assert_eq!(
            reported("Die Kinder, dessen Eltern arbeiten, bleiben länger."),
            ["dessen"]
        );
    }

    #[test]
    fn a_fitting_pronoun_or_no_relative_clause_is_quiet() {
        for text in [
            "Das Mädchen, das dort spielt, ist meine Tochter.",
            "Die Frau, der ich helfe, ist nett.",
            "Das Mädchen, der Hund und die Katze spielen.",
            "Ich erinnere mich an den Tag, wo wir uns kennengelernt haben.",
            "Die Stadt, wo ich wohne, ist klein.",
            "Die Kinder, die dort spielen, lachen.",
            "Das Haus, dessen Dach brennt, steht leer.",
            "Die Frau, deren Mann krank ist, kommt nicht.",
            "Ich gab dem Kind, das dort stand, ein Eis.",
            "Man bezeichnet die Reaktion eines Tieres auf einen Reiz, die gleich erfolgt.",
            "Sie legen ihre Flügel über den Hinterleib, die diesen verdecken.",
            "Sie stehlen eine Lagekarte und ein Feuerzeug, die benötigt werden.",
        ] {
            assert!(reported(text).is_empty(), "{text}: {:?}", reported(text));
        }
    }
}
