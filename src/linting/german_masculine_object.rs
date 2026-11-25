//! *Ich habe ein Brief geschrieben*, *Gibt es hier ein Supermarkt?*: a masculine object in the
//! accusative takes *einen*.

use harper_core::{
    Document, Span, Token, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
};

use crate::grammar::noun_gender::NounGender;
use crate::grammar::noun_phrase::lowercase_of;
use crate::grammar::token_text::starts_uppercase;
use crate::morphology::GenderSet;

const HABEN: &[&str] = &["habe", "hast", "hat", "haben", "habt", "hatte", "hatten"];

/// Verbs whose noun phrase is a nominative: *Ich bin ein Lehrer*, *Er heißt ein Riese*.
const COPULAS: &[&str] = &[
    "bin",
    "bist",
    "ist",
    "sind",
    "seid",
    "war",
    "warst",
    "waren",
    "wart",
    "sei",
    "wäre",
    "wären",
    "bleibe",
    "bleibst",
    "bleibt",
    "bleiben",
    "blieb",
    "blieben",
    "heiße",
    "heißt",
    "heißen",
    "hieß",
    "scheint",
    "scheinen",
    "schien",
    "gilt",
    "gelten",
    "sein",
    "gewesen",
    "geworden",
    "geblieben",
    "werden",
    "worden",
    "wurde",
    "wurden",
    "wurdest",
    "wurdet",
];

/// Auxiliaries and modals: a verb at the end of the clause may still follow the object
/// (*Ich möchte ein Kaffee trinken*). *werden* needs that verb, or it is the copula.
const AUXILIARIES: &[&str] = &[
    "habe",
    "hast",
    "hat",
    "haben",
    "habt",
    "hatte",
    "hatten",
    "hätte",
    "hättest",
    "hätten",
    "möchte",
    "möchtest",
    "möchten",
    "möchtet",
    "will",
    "willst",
    "wollen",
    "wollt",
    "wollte",
    "kann",
    "kannst",
    "können",
    "könnt",
    "konnte",
    "könnte",
    "muss",
    "musst",
    "müssen",
    "müsst",
    "soll",
    "sollst",
    "sollen",
    "sollt",
    "sollte",
    "darf",
    "darfst",
    "dürfen",
    "dürft",
    "mag",
    "magst",
    "mögen",
    "werde",
    "wirst",
    "wird",
    "werdet",
    "würde",
    "würdest",
    "würden",
    "würdet",
];

/// The nominative personal pronouns that can only be the subject. *es* is left out: *Es
/// kommt ein Zug* has the noun as its subject.
const SUBJECT_PRONOUNS: &[&str] = &["ich", "du", "er", "wir", "ihr", "sie", "man"];

/// Separable particles that close a clause behind the object: *Ich rufe ein Kollege an*.
const PARTICLES: &[&str] = &[
    "an", "auf", "aus", "ein", "mit", "vor", "zu", "zurück", "weg", "fest", "los", "her", "hin",
    "ab", "bei", "nach", "durch",
];

/// The nominative of the articles whose masculine accusative adds *-en*.
const ARTICLES: &[(&str, &str)] = &[
    ("ein", "einen"),
    ("kein", "keinen"),
    ("mein", "meinen"),
    ("dein", "deinen"),
    ("sein", "seinen"),
    ("unser", "unseren"),
    ("euer", "euren"),
];

/// Reports *ein*, *kein*, *mein* … in front of a noun that is only masculine, when the noun is
/// the object of
///
/// * a perfect with *haben*, right in front of its participle: *habe gestern ein Brief
///   geschrieben*;
/// * *es gibt*: *Gibt es hier ein Supermarkt?*.
///
/// Everything else is left to the nominative: *Das ist ein Brief*, *Ein Brief kam*.
#[derive(Default)]
pub struct GermanMasculineObject {
    nouns: NounGender,
}

impl GermanMasculineObject {
    /// The genders of the noun, or of the longest known word it ends on when the noun is a
    /// compound the dictionary does not list: *Supermarkt* → *Markt*.
    fn genders(&self, noun: &str) -> GenderSet {
        let known = self.nouns.genders(noun);
        if !known.is_empty() {
            return known;
        }
        let chars: Vec<char> = noun.chars().collect();
        for from in 1..chars.len().saturating_sub(3) {
            let mut tail: Vec<char> = chars[from..].to_vec();
            if let Some(first) = tail.first_mut() {
                *first = first.to_uppercase().next().unwrap_or(*first);
            }
            let genders = self.nouns.genders(&tail.iter().collect::<String>());
            if !genders.is_empty() {
                return genders;
            }
        }
        GenderSet::empty()
    }
}

impl GermanMasculineObject {
    /// *Ich hätte gerne ein Kaffee*, *Brauchst du ein Termin?*, *Er kauft ein Tisch*: a
    /// pronoun next to a verb that is no copula is the subject, so the noun phrase that ends
    /// the clause can only be its object. Between the verb and the article only small
    /// lowercase words may stand (*gerne*, *mir*, *noch*), behind the noun only the end of
    /// the clause, a separable particle, or the verbs an auxiliary governs.
    fn object_of_pronoun_subject(
        words: &[&Token],
        lower: &[String],
        at: usize,
        noun_at: usize,
        document: &Document,
    ) -> bool {
        let stops = |w: &str| {
            SUBJECT_PRONOUNS.contains(&w)
                || matches!(
                    w,
                    "es" | "und"
                        | "oder"
                        | "aber"
                        | "dass"
                        | "weil"
                        | "ob"
                        | "wenn"
                        | "als"
                        | "wie"
                        | "denn"
                        | "sondern"
                        | "so"
                        | "nicht"
                        | "nur"
                )
        };
        let mut j = at;
        while j > 0 {
            let t = words[j - 1];
            let w = lower[j - 1].as_str();
            if t.kind.is_punctuation()
                || t.kind.is_verb()
                || t.kind.is_preposition()
                || starts_uppercase(t, document)
                || stops(w)
            {
                break;
            }
            j -= 1;
        }
        if j < 2 {
            return false;
        }
        let (a, b) = (lower[j - 2].as_str(), lower[j - 1].as_str());
        let verb = if SUBJECT_PRONOUNS.contains(&a) && words[j - 1].kind.is_verb() {
            b
        } else if SUBJECT_PRONOUNS.contains(&b) && words[j - 2].kind.is_verb() {
            a
        } else {
            return false;
        };
        if COPULAS.contains(&verb) {
            return false;
        }
        // The pronoun opens the sentence or follows its verb: *Gestern sah er ein Hund* has
        // *er* behind the verb; *weil er ein Hund* would have it behind a conjunction.
        let first = j - 2;
        if first > 0 && !words[first - 1].kind.is_punctuation() && !words[first].kind.is_verb() {
            let before = lower[first - 1].as_str();
            if !(words[first - 1].kind.is_adverb() || matches!(before, "dann" | "jetzt" | "heute"))
            {
                return false;
            }
        }
        let tail: Vec<&str> = (noun_at + 1..words.len())
            .take_while(|&k| !words[k].kind.is_punctuation())
            .map(|k| lower[k].as_str())
            .collect();
        let is_aux = AUXILIARIES.contains(&verb);
        let needs_verb = matches!(
            verb,
            "werde" | "wirst" | "wird" | "werdet" | "würde" | "würdest" | "würden" | "würdet"
        );
        // *Er hat ein Termin beim Zahnarzt*: a prepositional phrase behind the object. Without
        // an auxiliary in front, a verb at its end would start a clause of its own (*Ich glaube
        // ein Mann im Park kommt*).
        let preposition_first = (noun_at + 1 < words.len())
            && (words[noun_at + 1].kind.is_preposition()
                || matches!(
                    tail.first().copied(),
                    Some("beim" | "im" | "am" | "zum" | "zur" | "vom" | "ins" | "ans")
                ));
        if preposition_first && !tail.is_empty() && !needs_verb {
            let last = words[noun_at + tail.len()];
            let verb_last = last.kind.is_verb() && !starts_uppercase(last, document);
            return is_aux || !verb_last || PARTICLES.contains(tail.last().unwrap_or(&""));
        }
        match tail.as_slice() {
            [] => !needs_verb,
            [p] if !is_aux => PARTICLES.contains(p),
            rest if is_aux && rest.len() <= 2 => rest.iter().enumerate().all(|(i, w)| {
                let token = words[noun_at + 1 + i];
                token.kind.is_verb()
                    && !starts_uppercase(token, document)
                    && !COPULAS.contains(w)
                    && (w.ends_with("en")
                        || w.ends_with('t')
                        || w.ends_with("ern")
                        || w.ends_with("eln"))
            }),
            _ => false,
        }
    }
}

impl Linter for GermanMasculineObject {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            let lower: Vec<String> = words.iter().map(|t| lowercase_of(t, document)).collect();
            let has_haben = lower.iter().any(|w| HABEN.contains(&w.as_str()));
            let has_gibt = lower
                .windows(2)
                .any(|w| (w[0] == "gibt" && w[1] == "es") || (w[0] == "es" && w[1] == "gibt"));
            for at in 1..words.len().saturating_sub(1) {
                let Some((_, accusative)) = ARTICLES.iter().find(|(n, _)| *n == lower[at]) else {
                    continue;
                };
                // A preposition or a genitive article in front: not an object.
                if (words[at - 1].kind.is_preposition()
                    && !starts_uppercase(words[at - 1], document))
                    || lower[at - 1] == "ist"
                {
                    continue;
                }
                // *ein großer Hund*: an adjective in the strong masculine nominative between
                // the article and the noun changes with it, to *einen großen Hund*.
                let adjective = (at + 2 < words.len()
                    && words[at + 1].kind.is_adjective()
                    && !starts_uppercase(words[at + 1], document)
                    && lower[at + 1].ends_with("er")
                    && lower[at + 1].chars().count() >= 4)
                    .then(|| document.get_span_content_str(&words[at + 1].span));
                let noun_at = if adjective.is_some() { at + 2 } else { at + 1 };
                let noun = words[noun_at];
                if !starts_uppercase(noun, document) || !noun.kind.is_word() {
                    continue;
                }
                let text = document.get_span_content_str(&noun.span);
                if self.genders(&text) != GenderSet::MASCULINE {
                    continue;
                }
                let after = words.get(noun_at + 1).map(|t| lowercase_of(t, document));
                let participle = after.as_deref().is_some_and(|w| {
                    w.chars().count() >= 6
                        && ["ge", "ver", "be", "er", "ent", "zer", "über"]
                            .iter()
                            .any(|prefix| w.starts_with(prefix))
                        && (w.ends_with('t') || w.ends_with("en"))
                }) && words[noun_at + 1].kind.is_verb();
                // The auxiliary stands in the same clause: no comma between them.
                let same_clause = lower[..at]
                    .iter()
                    .rposition(|w| HABEN.contains(&w.as_str()))
                    .is_some_and(|h| {
                        // The subject stands in front of *haben* (*Ich habe gestern ein Brief
                        // geschrieben*); behind it, *ein Zyklon* may be the subject itself.
                        let pronoun = |w: &str| {
                            matches!(
                                w,
                                "ich" | "du" | "er" | "sie" | "es" | "wir" | "ihr" | "man"
                            )
                        };
                        // *Hast du dein Schlüssel vergessen?*: the question has the pronoun behind.
                        (h > 0 && pronoun(lower[h - 1].as_str())
                            || h == 0 && lower.get(1).is_some_and(|w| pronoun(w)))
                            && !words[h..at].iter().any(|t| t.kind.is_comma())
                    });
                let ends_question = has_gibt
                    && words
                        .get(noun_at + 1)
                        .is_none_or(|t| t.kind.is_punctuation());
                // *Die Wohnung hat zwei Schlafzimmer und ein Balkon.*: a second object of *haben*
                // behind *und*, at the end of the clause.
                let second_object = at >= 3
                    && lower[at - 1] == "und"
                    && words
                        .get(noun_at + 1)
                        .is_none_or(|t| t.kind.is_punctuation())
                    && lower[..at - 1]
                        .iter()
                        .rposition(|w| matches!(w.as_str(), "hat" | "haben" | "hast" | "habe"))
                        .is_some_and(|h| {
                            at - 1 - h >= 2 && !words[h..at].iter().any(|t| t.kind.is_comma())
                        });
                if !(has_haben && participle && same_clause
                    || ends_question
                    || second_object
                    || Self::object_of_pronoun_subject(&words, &lower, at, noun_at, document))
                {
                    continue;
                }
                let (end, replacement) = match &adjective {
                    Some(adjective) => {
                        let stem = &adjective[..adjective.len() - 2];
                        (words[at + 1].span.end, format!("{accusative} {stem}en"))
                    }
                    None => (words[at].span.end, accusative.to_string()),
                };
                lints.push(Lint {
                    span: Span::new(words[at].span.start, end),
                    lint_kind: LintKind::Agreement,
                    suggestions: vec![Suggestion::ReplaceWith(replacement.chars().collect())],
                    message: format!(
                        "»{text}« ist hier das Objekt im Akkusativ: »{replacement} {text}«."
                    ),
                    priority: 31,
                });
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Setzt »einen« vor ein männliches Objekt im Akkusativ (»ein Brief geschrieben«)."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanMasculineObject;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn count(text: &str) -> usize {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanMasculineObject::default().lint(&document).len()
    }

    #[test]
    fn a_masculine_object_takes_einen() {
        for text in [
            "Ich habe gestern ein Brief geschrieben.",
            "Er hat sein Hund gesehen.",
            "Wir haben unser Termin verschoben.",
            "Hast du dein Schlüssel vergessen?",
            "Die Wohnung hat zwei Schlafzimmer und ein Balkon.",
            "Gibt es hier in der Nähe ein Supermarkt?",
            "Es gibt kein Bahnhof.",
            "Ich hätte gerne ein Kaffee.",
            "Ich möchte ein Kaffee trinken.",
            "Brauchst du ein Termin?",
            "Wir suchen ein Mitarbeiter.",
            "Dann kaufe ich ein Tisch.",
            "Ich rufe morgen ein Kollege an.",
            "Er kennt ein Arzt.",
            "Er hat ein Termin beim Zahnarzt.",
            "Ich sehe ein Mann im Park.",
            "Ich sehe ein großer Hund.",
            "Wir suchen ein neuer Mitarbeiter.",
        ] {
            assert_eq!(count(text), 1, "{text}");
        }
    }

    #[test]
    fn other_cases_are_quiet() {
        for text in [
            "Ich habe gestern einen Brief geschrieben.",
            "Das ist ein Brief.",
            "Ein Brief ist gekommen.",
            "Ich habe ein Buch gelesen.",
            "Er hat ein Haus gekauft.",
            "Gibt es hier ein Hotel?",
            "Ich habe mit ein Brief gespielt.",
            "Es gibt ein Problem.",
            "Ich habe ein Kind gesehen.",
            "Wir haben unser Ziel erreicht.",
            "Er hat ein Haus und ein Auto.",
            "Sie hat Besuch und ein Kollege kommt.",
            "Ein Hunger hat alle ergriffen, ein Rausch geht durch die Welt.",
            "Ich bin ein Lehrer.",
            "Ich werde ein Lehrer.",
            "Er möchte ein Lehrer werden.",
            "Ich glaube ein Mann kommt.",
            "Es kommt ein Zug.",
            "Er arbeitet als ein Koch.",
            "Ich und ein Freund kommen.",
            "Ich hätte gerne einen Kaffee.",
            "Ich möchte ein Bier.",
            "Er bleibt ein Freund.",
            "Weil er ein Hund hat.",
            "Ich weiß, dass ein Mann kommt.",
            "Wir gehen ein Stück.",
            "Ich glaube ein Mann im Park kommt.",
            "Er hat einen Termin beim Zahnarzt.",
            "Ich sehe einen großen Hund.",
            "Das ist ein großer Hund.",
            "Ein großer Hund bellt.",
            "Ich glaube ein großer Hund kommt.",
            "Außerdem wurde er ein bekannter Sportfunktionär.",
        ] {
            assert_eq!(count(text), 0, "{text}");
        }
    }
}
