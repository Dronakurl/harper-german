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

impl GermanVorfeldComma {
    /// *Leider, kann ich nicht kommen*: a sentence adverb alone in front of
    /// the verb is the whole first phrase, and takes no comma. Interjections
    /// and answers (*Ja, ich komme*, *Oh, das wusste ich nicht*) put a whole
    /// clause behind them, with a first phrase of its own, and are not here.
    fn adverb_comma(words: &[&Token], document: &Document) -> Option<Lint> {
        const SENTENCE_ADVERBS: &[&str] = &[
            "leider",
            "natürlich",
            "trotzdem",
            "deshalb",
            "deswegen",
            "daher",
            "vielleicht",
            "hoffentlich",
            "eigentlich",
            "heute",
            "gestern",
            "morgen",
            "jetzt",
            "dann",
            "danach",
            "außerdem",
            "selbstverständlich",
            "glücklicherweise",
            "wahrscheinlich",
            "sicherlich",
            "bestimmt",
            "anschließend",
            "schließlich",
            "zuerst",
            "manchmal",
            "oft",
        ];
        let [adverb, comma, verb, subject, ..] = words else {
            return None;
        };
        if !SENTENCE_ADVERBS.contains(&Self::lower(adverb, document).as_str())
            || !matches!(comma.kind, TokenKind::Punctuation(Punctuation::Comma))
            || Self::capitalized(verb, document)
            || !Self::verb_only(verb, document)
            || !PRONOUNS.contains(&Self::lower(subject, document).as_str())
        {
            return None;
        }
        Some(Lint {
            span: comma.span,
            lint_kind: LintKind::Punctuation,
            suggestions: vec![Suggestion::Remove],
            priority: 28,
            message: format!(
                "»{}« ist der ganze erste Satzteil. Zwischen ihm und dem Verb steht kein Komma.",
                document.get_span_content_str(&adverb.span)
            ),
        })
    }

    /// *Brot, Butter, und Käse*: in a list of single nouns the last two are
    /// joined by *und* alone.
    fn list_comma(words: &[&Token], document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for at in 3..words.len().saturating_sub(2) {
            let (first, comma1, second, comma2, und, last) = (
                words[at - 3],
                words[at - 2],
                words[at - 1],
                words[at],
                words[at + 1],
                words[at + 2],
            );
            let noun = |t: &Token| {
                matches!(t.kind, TokenKind::Word(_))
                    && Self::capitalized(t, document)
                    && !t.kind.is_pronoun()
                    && !t.kind.is_determiner()
                    && !PRONOUNS.contains(&Self::lower(t, document).as_str())
            };
            let comma = |t: &Token| matches!(t.kind, TokenKind::Punctuation(Punctuation::Comma));
            // The list ends the sentence: nothing but its last noun behind *und*.
            let ends = words.get(at + 3).is_none_or(|t| {
                matches!(
                    t.kind,
                    TokenKind::Punctuation(
                        Punctuation::Period | Punctuation::Bang | Punctuation::Question
                    )
                )
            });
            if noun(first)
                && comma(comma1)
                && noun(second)
                && comma(comma2)
                && matches!(
                    Self::lower(und, document).as_str(),
                    "und" | "oder" | "sowie"
                )
                && noun(last)
                && ends
            {
                lints.push(Lint {
                    span: comma2.span,
                    lint_kind: LintKind::Punctuation,
                    suggestions: vec![Suggestion::Remove],
                    priority: 28,
                    message: format!(
                        "Vor dem »{}« am Ende einer Aufzählung steht kein Komma.",
                        document.get_span_content_str(&und.span)
                    ),
                });
            }
        }
        lints
    }
}

impl GermanVorfeldComma {
    fn remove(comma: &Token, message: &str) -> Lint {
        Lint {
            span: comma.span,
            lint_kind: LintKind::Punctuation,
            suggestions: vec![Suggestion::Remove],
            priority: 28,
            message: message.to_owned(),
        }
    }

    /// Commas inside a comparison or a paired conjunction, which join no clauses:
    ///
    /// * *Sowohl Peter, als auch Maria*: *sowohl … als auch* joins two parts of one clause;
    /// * *besser, als erwartet*: a comparative and *als* with a participle or an adverb that
    ///   ends the sentence;
    /// * *Ich habe, das Buch gelesen*: a comma between the auxiliary in second place and a
    ///   phrase that ends on the participle.
    fn comparison_comma(words: &[&Token], document: &Document) -> Vec<Lint> {
        const CLOSERS: &[&str] = &[
            "erwartet",
            "gedacht",
            "befürchtet",
            "geplant",
            "vorgesehen",
            "üblich",
            "sonst",
            "vorher",
            "früher",
            "gewohnt",
            "nötig",
            "notwendig",
            "angenommen",
            "erhofft",
        ];
        const AUXILIARIES: &[&str] = &[
            "habe", "hast", "hat", "haben", "habt", "hatte", "hatten", "bin", "bist", "ist",
            "sind", "seid", "war", "waren",
        ];
        let comma = |t: &Token| matches!(t.kind, TokenKind::Punctuation(Punctuation::Comma));
        let ends = |at: usize| {
            words.get(at).is_none_or(|t| {
                matches!(
                    t.kind,
                    TokenKind::Punctuation(
                        Punctuation::Period | Punctuation::Bang | Punctuation::Question
                    )
                )
            })
        };
        let mut lints = Vec::new();
        let has_sowohl = words.iter().any(|t| Self::lower(t, document) == "sowohl");
        for at in 1..words.len().saturating_sub(2) {
            if !comma(words[at]) {
                continue;
            }
            let next = Self::lower(words[at + 1], document);
            let after = Self::lower(words[at + 2], document);
            if has_sowohl && next == "als" && after == "auch" {
                lints.push(Self::remove(
                    words[at],
                    "In »sowohl … als auch« steht vor »als auch« kein Komma.",
                ));
                continue;
            }
            let previous = Self::lower(words[at - 1], document);
            let comparative = previous.ends_with("er")
                && !Self::capitalized(words[at - 1], document)
                && words[at - 1].kind.is_adjective();
            if comparative && next == "als" && CLOSERS.contains(&after.as_str()) && ends(at + 3) {
                lints.push(Self::remove(
                    words[at],
                    "Ein Vergleich mit »als« ohne eigenes Verb wird nicht mit Komma abgetrennt.",
                ));
                continue;
            }
            // *Ich habe, das Buch gelesen.*
            if at == 2
                && AUXILIARIES.contains(&previous.as_str())
                && words[0].kind.is_pronoun()
                && matches!(
                    next.as_str(),
                    "das" | "den" | "die" | "ein" | "eine" | "einen"
                )
                && let Some(last) = words.len().checked_sub(2)
                && last > at + 2
                && ends(last + 1)
                && {
                    let participle = Self::lower(words[last], document);
                    participle.starts_with("ge")
                        && (participle.ends_with('t') || participle.ends_with("en"))
                }
                && !words[at + 1..last].iter().any(|t| {
                    comma(t)
                        || (t.kind.is_verb()
                            && !Self::capitalized(t, document)
                            && !t.kind.is_determiner())
                })
            {
                lints.push(Self::remove(
                    words[at],
                    "Zwischen dem Hilfsverb und seinem Objekt steht kein Komma.",
                ));
            }
        }
        lints
    }
}

impl Linter for GermanVorfeldComma {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|t| !t.kind.is_whitespace())
                .collect();
            lints.extend(Self::check(&words, document));
            lints.extend(Self::adverb_comma(&words, document));
            lints.extend(Self::list_comma(&words, document));
            lints.extend(Self::comparison_comma(&words, document));
        }
        lints
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
    fn a_comma_inside_a_comparison_or_pair_is_reported() {
        for text in [
            "Sowohl Peter, als auch Maria kommen.",
            "Das Ergebnis war besser, als erwartet.",
            "Es dauerte länger, als gedacht.",
            "Ich habe, das Buch gelesen.",
        ] {
            assert_eq!(count(text), 1, "{text}");
        }
        for text in [
            "Er ist größer, als ich dachte.",
            "Das Ergebnis war besser als erwartet.",
            "Ich habe, wie gesagt, das Buch gelesen.",
            "Ich glaube, das Buch ist gut.",
            "Sowohl Peter als auch Maria kommen.",
            "Ich habe, das weißt du, keine Zeit.",
        ] {
            assert_eq!(count(text), 0, "{text}");
        }
    }

    #[test]
    fn a_comma_before_the_verb_is_reported() {
        for text in [
            "Nach einer seit 2015 andauernden Pause, trat die Band wieder auf.",
            "Mit zwei elften Plätzen als bestem Ergebnis und 16 Punkten am Saisonende, konnte er sich halten.",
            "Das Fachwerkhaus, erinnerte ihn an das Haus der sieben Zwerge.",
            "Im Jahr 2020, wurde er Meister.",
            "Leider, kann ich nicht kommen.",
            "Natürlich, helfe ich dir.",
            "Sie kaufte Brot, Butter, und Käse.",
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
            "Ja, ich komme gerne.",
            "Nein danke, ich habe keinen Hunger.",
            "Oh, das wusste ich nicht.",
            "Sie kaufte Brot, Butter und Käse.",
            "Er sagte, dass er kommt, und dass er Wein mitbringt.",
            "Peter, Anna, und ich gehen.",
            "Mein Bruder, Peter und ich gehen.",
            "Bei der Meisterschaft erreichte Chetri das Finale, verlor dort aber gegen die Malediven.",
            "Seit 1943 wieder im Stadtparlament, war er Mitglied der Delegation.",
        ] {
            assert_eq!(count(text), 0, "{text}");
        }
    }
}
