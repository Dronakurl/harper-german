//! A comma between the first phrase of a main clause and its verb: *Nach einer
//! langen Pause**,** trat die Band wieder auf*, *Das Fachwerkhaus**,** erinnerte
//! ihn an …*.

use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Punctuation, Token, TokenKind, TokenStringExt};

use crate::grammar::determiners::determiner_readings;
use crate::linting::german_relative_clause_comma::GermanRelativeClauseComma;
use crate::linting::german_subordinate_comma::SUBORDINATORS;

/// Words that open a clause or a group of their own in front of the verb: *Als
/// er kam, …*, *Um zu gewinnen, …*, *Wie gesagt, …*.
const OPENERS: &[&str] = &[
    "als",
    "wenn",
    "wie",
    "ob",
    "um",
    "ohne",
    "statt",
    "anstatt",
    "je",
    "und",
    "oder",
    "aber",
    "sondern",
    "denn",
    "ja",
    "nein",
    "doch",
    "also",
    "kurz",
    "allerdings",
    "übrigens",
];

/// The salutation of a letter or a call: *Liebe Kollegen, habt ihr …*.
const SALUTATIONS: &[&str] = &[
    "liebe", "lieber", "liebes", "hallo", "hi", "sehr", "geehrte", "geehrter", "hey", "moin",
    "servus", "grüß", "guten",
];

/// What stands behind the verb of the main clause: its subject, or a pronoun
/// object when the first phrase is the subject (*erinnerte **ihn***).
const PRONOUNS: &[&str] = &[
    "ich", "du", "er", "sie", "es", "wir", "ihr", "man", "ihn", "ihm", "ihnen", "mich", "mir",
    "dich", "dir", "uns", "euch", "sich",
];

/// The longest first phrase, in words, that is looked at.
const MAX_VORFELD: usize = 12;

/// German main clauses put exactly one phrase in front of the verb, and no
/// comma between them. The pattern is kept narrow:
///
/// * the phrase before the comma opens the sentence, has two words or more,
///   holds no verb and ends on a noun or a number. A subordinate clause (*Als
///   er kam*), an infinitive group (*Um zu gewinnen*) and a participle group
///   (*Vom Regen durchnässt*) all hold or end on a verb;
/// * behind the comma comes a lower-case finite verb and then a pronoun, an
///   article or a capitalized word, the rest of a main clause. An apposition
///   (*Das Haus, ein Fachwerkbau, …*) and a list (*Äpfel, Birnen*) have no verb
///   there;
/// * not in a question or an exclamation, and not after a salutation: *Liebe
///   Kollegen, habt ihr …?* puts the verb first after a vocative.
#[derive(Default)]
pub struct GermanVorfeldComma;

impl GermanVorfeldComma {
    fn lower(token: &Token, document: &Document) -> String {
        document
            .get_span_content(&token.span)
            .iter()
            .flat_map(|c| c.to_lowercase())
            .collect()
    }

    fn capitalized(token: &Token, document: &Document) -> bool {
        document
            .get_span_content(&token.span)
            .first()
            .is_some_and(|c| c.is_uppercase())
    }

    /// A word that is a verb and nothing that could stand in a noun phrase.
    fn verb_only(token: &Token, document: &Document) -> bool {
        let TokenKind::Word(metadata) = &token.kind else {
            return false;
        };
        GermanRelativeClauseComma::is_verb_token(token, document)
            && !metadata
                .as_ref()
                .is_some_and(|m| m.is_adjective() || m.is_adverb())
    }

    fn check(words: &[&Token], document: &Document) -> Option<Lint> {
        let comma_at = words
            .iter()
            .position(|t| !matches!(t.kind, TokenKind::Word(_) | TokenKind::Number(_)))?;
        if !matches!(
            words[comma_at].kind,
            TokenKind::Punctuation(Punctuation::Comma)
        ) || !(2..=MAX_VORFELD).contains(&comma_at)
        {
            return None;
        }
        // A question or an exclamation may put the verb first.
        if words.last().is_some_and(|t| {
            matches!(
                t.kind,
                TokenKind::Punctuation(Punctuation::Question | Punctuation::Bang)
            )
        }) {
            return None;
        }
        let vorfeld = &words[..comma_at];
        let first = Self::lower(vorfeld[0], document);
        if OPENERS.contains(&first.as_str())
            || SALUTATIONS.contains(&first.as_str())
            || SUBORDINATORS.contains(&first.as_str())
        {
            return None;
        }
        // A capitalized first word has to be a noun or a name, not an adjective,
        // a participle or an adverb (*Geboren in Mexiko, …*, *Klein von
        // Gestalt, …*), and not a verb (*Fällt die Entlassung auf …, kann …*).
        let first_is_noun = Self::capitalized(vorfeld[0], document)
            && match &vorfeld[0].kind {
                TokenKind::Word(Some(m)) => {
                    !m.is_adjective() && !m.is_verb() && !m.is_adverb() && !m.is_conjunction()
                }
                _ => false,
            };
        let first_fits = vorfeld[0].kind.is_preposition()
            || determiner_readings(&first).is_some()
            || first_is_noun;
        let last = vorfeld[comma_at - 1];
        let last_fits = matches!(last.kind, TokenKind::Number(_))
            || (matches!(last.kind, TokenKind::Word(_)) && Self::capitalized(last, document));
        if !first_fits
            || !last_fits
            || vorfeld.iter().enumerate().any(|(i, t)| {
                Self::verb_only(t, document)
                    // A verb that also reads as an adjective counts only as an
                    // attribute, in front of its noun (*andauernden Pause*); else
                    // the phrase is a clause of its own (*Rabin gefährdet das …*).
                    // It has to stand inside a noun phrase that an article opened,
                    // with no noun between the two: *einer seit 2015 andauernden
                    // Pause*, but not *der Meisterschaft erreichte Chetri*.
                    || (GermanRelativeClauseComma::is_verb_token(t, document)
                        && !(vorfeld
                            .get(i + 1)
                            .is_some_and(|next| Self::capitalized(next, document))
                            && vorfeld[..i]
                                .iter()
                                .rposition(|w| {
                                    determiner_readings(&Self::lower(w, document)).is_some()
                                })
                                .is_some_and(|d| {
                                    !vorfeld[d + 1..i]
                                        .iter()
                                        .any(|w| Self::capitalized(w, document))
                                })))
                    // *Seit 1943 wieder im Stadtparlament, war er …*: an adverb
                    // makes the phrase a predicate of its own, without its
                    // participle, and the comma may stand.
                    || matches!(
                        Self::lower(t, document).as_str(),
                        "wieder" | "noch" | "schon" | "bereits" | "erst" | "nur" | "kaum" | "fast"
                            | "einst" | "später" | "zuvor" | "inzwischen"
                    )
                    // *Um zu gewinnen*: an infinitive group. The preposition *zu*
                    // has its noun behind it (*zu Beginn der Saison*).
                    || (Self::lower(t, document) == "zu"
                        && !vorfeld
                            .iter()
                            .skip_while(|w| w.span != t.span)
                            .nth(1)
                            .is_some_and(|w| {
                                Self::capitalized(w, document)
                                    || determiner_readings(&Self::lower(w, document)).is_some()
                            }))
            })
        {
            return None;
        }

        let verb = words.get(comma_at + 1)?;
        let after = words.get(comma_at + 2)?;
        if Self::capitalized(verb, document)
            || !GermanRelativeClauseComma::is_verb_token(verb, document)
        {
            return None;
        }
        let after_lower = Self::lower(after, document);
        let pronoun_follows = PRONOUNS.contains(&after_lower.as_str());
        // A weak preterite reads as a participle adjective too (*erinnerte*);
        // then only a pronoun behind it settles the verb.
        // *Herr Müller, kommen Sie bitte*: a vocative and the polite imperative.
        let polite_imperative = document.get_span_content_str(&after.span) == "Sie"
            && Self::lower(verb, document).ends_with("en");
        // A verb with no other reading (*wohnt*, *fährt*, *hat*) is finite
        // whatever follows it, short of a coordination or another comma:
        // *Meine Schwester, wohnt in Berlin*.
        // *…, verlor dort aber mit 0:1*, *…, diente aber auch als …*: a second
        // predicate of the same subject, joined by *aber*.
        if words[comma_at + 1..]
            .iter()
            .take(4)
            .any(|t| matches!(Self::lower(t, document).as_str(), "aber" | "jedoch"))
        {
            return None;
        }
        let surely_finite = Self::verb_only(verb, document)
            && !verb.kind.is_noun()
            && matches!(after.kind, TokenKind::Word(_))
            && !matches!(after_lower.as_str(), "und" | "oder" | "sowie");
        let main_clause_goes_on = !polite_imperative
            && (pronoun_follows
                || surely_finite
                || (Self::verb_only(verb, document)
                    && (determiner_readings(&after_lower).is_some()
                        || (matches!(after.kind, TokenKind::Word(_))
                            && Self::capitalized(after, document)))));
        if !main_clause_goes_on {
            return None;
        }

        Some(Lint {
            span: words[comma_at].span,
            lint_kind: LintKind::Punctuation,
            suggestions: vec![Suggestion::Remove],
            priority: 28,
            message: "Zwischen dem ersten Satzteil und dem Verb steht kein Komma.".to_owned(),
        })
    }
}

impl Linter for GermanVorfeldComma {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        document
            .iter_sentences()
            .filter_map(|sentence| {
                let words: Vec<&Token> = sentence
                    .iter()
                    .filter(|t| !t.kind.is_whitespace())
                    .collect();
                Self::check(&words, document)
            })
            .collect()
    }

    fn description(&self) -> &str {
        "Findet ein Komma zwischen dem ersten Satzteil und dem Verb: »Nach der Pause, trat die Band auf«."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanVorfeldComma;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn count(text: &str) -> usize {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanVorfeldComma.lint(&document).len()
    }

    #[test]
    fn a_comma_before_the_verb_is_reported() {
        for text in [
            "Nach einer seit 2015 andauernden Pause, trat die Band wieder auf.",
            "Mit zwei elften Plätzen als bestem Ergebnis und 16 Punkten am Saisonende, konnte er sich halten.",
            "Das Fachwerkhaus, erinnerte ihn an das Haus der sieben Zwerge.",
            "Im Jahr 2020, wurde er Meister.",
            "Nach seinem ersten öffentlichen Auftritt 1955, entschied er sich für das Santurspiel.",
            "Aufgrund antiker Fundstücke während der Bauarbeiten, kam es zu Unterbrechungen.",
            "Meine Schwester, wohnt in Berlin.",
            "Der Zug, fährt um acht ab.",
            "Mein Bruder, arbeitet viel.",
        ] {
            assert_eq!(count(text), 1, "{text}");
        }
    }

    #[test]
    fn the_comma_of_a_clause_or_group_is_left_alone() {
        for text in [
            "Als er kam, ging sie.",
            "Um zu gewinnen, trainierte er.",
            "Vom Regen durchnässt, kam er heim.",
            "Das Haus, ein Fachwerkbau, erinnerte ihn an früher.",
            "Äpfel, Birnen und Pflaumen liegen dort.",
            "Peter, komm her.",
            "Liebe Kollegen, habt ihr Zeit?",
            "Sehr geehrte Damen und Herren, wir freuen uns.",
            "Nach einer langen Pause trat die Band wieder auf.",
            "Wie gesagt, kommt er morgen.",
            "Kurz gesagt, war es gut.",
            "Der Mann, den ich kenne, wohnt hier.",
            "Im Jahr 2020, als er Meister wurde, war er jung.",
            "Rabin gefährdet das Leben der Juden, schafft einen Terrorstaat.",
            "Geboren in Mexiko, wuchs Wizan in Los Angeles auf.",
            "Klein von Gestalt, war sie groß durch ihre Tugenden.",
            "Fällt die Entlassung auf ein Wochenende, kann der Termin vorgezogen werden.",
            "Gerber absolvierte 22 Spiele, hatte eine erstklassige Fangquote.",
            "Selbst der geringste Bauer, schrieb der Professor, wusste das.",
            "Herr Müller, kommen Sie bitte herein.",
            "Mein Bruder, Peter und ich gehen.",
            "Bei der Meisterschaft erreichte Chetri das Finale, verlor dort aber gegen die Malediven.",
            "Seit 1943 wieder im Stadtparlament, war er Mitglied der Delegation.",
        ] {
            assert_eq!(count(text), 0, "{text}");
        }
    }
}
