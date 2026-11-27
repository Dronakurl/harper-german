//! *Er kann das nicht gemacht*, *Sie hat das Buch lesen*: the verb at the end
//! of the clause in the form the finite verb in front does not take.

use crate::morphology::{Gender, GenderSet};
use harper_core::{
    Document, Punctuation, Token, TokenKind, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
};

use crate::grammar::token_text::starts_lowercase;
use crate::grammar::verbs::lemma_of_participle;
use crate::linting::german_perfect_auxiliary::SEIN_PARTICIPLES;
use crate::{
    grammar::{
        noun_gender::NounGender,
        noun_phrase::lowercase_of,
        subjects::irregular_finite_verb_lemma,
        verbs::{
            has_zu_infix, infinitive_of_participle, is_participle_not_infinitive,
            is_past_participle, looks_like_participle, participle_spelled_like_infinitive,
        },
    },
    linting::german_subordinate_comma::SUBORDINATORS,
};

/// Coordinating conjunctions: a new clause, or a second verb, starts behind
/// them.
const COORDINATORS: &[&str] = &["und", "oder", "aber", "sondern", "denn", "sowie"];

/// The modals that take a bare infinitive. *mögen* is left out: *Ich möchte
/// das Fenster geschlossen* puts a participle behind it as an attribute.
const MODALS: &[&str] = &["können", "müssen", "dürfen", "sollen", "wollen"];

/// Verbs that take a bare infinitive and, after *haben*, stand last in a
/// correct clause: the *Ersatzinfinitiv* (*hat kommen können*, *hat ihn gehen
/// lassen*) and *haben* with a verb of position (*Er hat Geld auf dem Konto
/// liegen*).
const INFINITIVE_LAST: &[&str] = &[
    "können", "müssen", "dürfen", "sollen", "wollen", "mögen", "lassen", "sehen", "hören",
    "helfen", "brauchen", "liegen", "stehen", "hängen", "stecken", "sitzen",
];

/// Reports the last verb of a main clause in the wrong one of its two
/// non-finite forms:
///
/// * a **participle behind a modal**, which takes the infinitive: *Man kann
///   die Größe nicht beliebig erhöht* → *erhöhen*, *Er will alles gemacht* →
///   *machen*;
/// * an **infinitive behind *haben***, which takes the participle: *Intel hat
///   den Termin erneut verschieben* → *verschoben*.
///
/// The clause runs from the finite verb to punctuation, a coordinating
/// conjunction or a subordinator, and its last word is the one checked. Left
/// alone are clauses with *werden*, *sein* or another *haben* (*kann gemacht
/// werden*, *muss erledigt sein*), the *Ersatzinfinitiv* and the verbs of
/// position (`INFINITIVE_LAST`), two infinitives in a row, a *zu* infinitive,
/// and *gut reden haben*. A form in *-en* that is also a strong participle
/// (*bekommen*, *vergessen*) is no infinitive for this purpose.
///
/// No replacement is offered: the infinitive of *gelegen* is *liegen*, the
/// participle of *verschieben* is *verschoben*, and the dictionary does not
/// link them.
pub struct GermanVerbCluster {
    nouns: NounGender,
}

impl Default for GermanVerbCluster {
    fn default() -> Self {
        Self::new()
    }
}

impl GermanVerbCluster {
    pub fn new() -> Self {
        Self {
            nouns: NounGender::new(),
        }
    }

    /// Is the word written small a noun rather than an infinitive? A
    /// nominalized infinitive is neuter, so a noun of another gender is not
    /// one (*Sorgen*, *Schulden*, *Glauben*, *Schnupfen*), and neither is a
    /// word behind a determiner or an adjective (*das nachsehen*, *ganz
    /// andere sorgen*). Those are `GermanNounCapitalization`'s.
    fn is_a_noun_written_small(&self, word: &str, previous: &Token, document: &Document) -> bool {
        // A declined adjective, not an adverbial one: *hat erneut verschieben*
        // is a verb.
        let previous_word = lowercase_of(previous, document);
        let declined = previous.kind.is_adjective()
            && ["e", "en", "er", "es", "em"]
                .iter()
                .any(|ending| previous_word.ends_with(ending));
        if previous.kind.is_determiner() || declined {
            return true;
        }
        let mut capitalized = word.chars();
        let capitalized: String = capitalized
            .next()
            .map(|first| first.to_uppercase().chain(capitalized).collect())
            .unwrap_or_default();
        let genders = self.nouns.genders(&capitalized);
        !genders.is_empty() && genders != GenderSet::from(Gender::Neuter)
    }

    /// The clauses of a sentence that a clause-ending mark closes, as word
    /// indices.
    ///
    /// A clause closed by a coordinating conjunction is dropped: its last
    /// verb may share an auxiliary with the next one — *können exportiert und
    /// genutzt werden*. So is one cut short by a quotation mark or a bracket,
    /// whose last word is not the end of the clause: *den vorher abgemahnten
    /// „Kollegen“*.
    fn clauses(tokens: &[&Token], document: &Document) -> Vec<Vec<usize>> {
        let mut clauses = Vec::new();
        let mut current = Vec::new();
        for (at, token) in tokens.iter().enumerate() {
            match token.kind {
                TokenKind::Word(_) => {
                    let word = lowercase_of(token, document);
                    if Self::opens_relative_clause(tokens, at, document) {
                        clauses.push(std::mem::take(&mut current));
                    }
                    if COORDINATORS.contains(&word.as_str()) {
                        current = Vec::new();
                    } else if SUBORDINATORS.contains(&word.as_str()) || word == "wenn" {
                        clauses.push(std::mem::take(&mut current));
                    } else {
                        current.push(at);
                    }
                }
                TokenKind::Number(_) => current.push(at),
                TokenKind::Punctuation(Punctuation::Hyphen | Punctuation::Apostrophe) => {}
                // *kann nicht verkauft, sondern nur vererbt werden*: a comma
                // before a coordinator, with an auxiliary further on that both
                // participles share.
                TokenKind::Punctuation(Punctuation::Comma)
                    if Self::shares_a_later_auxiliary(&tokens[at + 1..], document) =>
                {
                    current = Vec::new();
                }
                TokenKind::Punctuation(
                    Punctuation::Period
                    | Punctuation::Comma
                    | Punctuation::Semicolon
                    | Punctuation::Colon
                    | Punctuation::Question
                    | Punctuation::Bang,
                ) => clauses.push(std::mem::take(&mut current)),
                TokenKind::Punctuation(_) | TokenKind::Unlintable => current = Vec::new(),
                _ => {}
            }
        }
        clauses.push(current);
        clauses
    }

    /// *… gekauft in dem wir wohnen*: a preposition, a relative pronoun and a
    /// subject pronoun open a relative clause even where its comma is missing.
    fn opens_relative_clause(tokens: &[&Token], at: usize, document: &Document) -> bool {
        const RELATIVES: &[&str] = &[
            "dem", "der", "den", "das", "denen", "deren", "dessen", "die",
        ];
        const SUBJECTS: &[&str] = &["ich", "du", "er", "sie", "es", "wir", "ihr", "man"];
        let word_at = |i: usize| {
            tokens
                .get(i)
                .filter(|t| matches!(t.kind, TokenKind::Word(_)))
                .map(|t| lowercase_of(t, document))
        };
        tokens[at].kind.is_preposition()
            && word_at(at + 1).is_some_and(|w| RELATIVES.contains(&w.as_str()))
            && word_at(at + 2).is_some_and(|w| SUBJECTS.contains(&w.as_str()))
    }

    /// Does the rest of the sentence begin with a coordinator and end its
    /// next clause with *werden*, *sein* or *haben*?
    fn shares_a_later_auxiliary(rest: &[&Token], document: &Document) -> bool {
        let mut words = rest.iter().take_while(|token| {
            !matches!(
                token.kind,
                TokenKind::Punctuation(
                    Punctuation::Period
                        | Punctuation::Comma
                        | Punctuation::Semicolon
                        | Punctuation::Question
                        | Punctuation::Bang
                )
            )
        });
        let starts_with_coordinator = words.next().is_some_and(|token| {
            matches!(token.kind, TokenKind::Word(_))
                && COORDINATORS.contains(&lowercase_of(token, document).as_str())
        });
        starts_with_coordinator
            && words.last().is_some_and(|token| {
                matches!(
                    lowercase_of(token, document).as_str(),
                    "werden" | "worden" | "sein" | "haben"
                )
            })
    }

    fn check(&self, clause: &[usize], tokens: &[&Token], document: &Document) -> Option<Lint> {
        let (&last_at, before) = clause.split_last()?;
        let last = tokens[last_at];
        if before.is_empty() || !starts_lowercase(last, document) {
            return None;
        }
        let last_word = lowercase_of(last, document);
        let words: Vec<String> = before
            .iter()
            .map(|&at| lowercase_of(tokens[at], document))
            .collect();
        let lemmas: Vec<Option<&str>> = words
            .iter()
            .map(|word| irregular_finite_verb_lemma(word))
            .collect();
        // A passive or a perfect infinitive takes the participle legitimately:
        // *kann gemacht werden*, *muss erledigt sein*, *soll gesehen haben*.
        if last_word == "werden" || last_word == "sein" || last_word == "haben" {
            return None;
        }
        // A finite form at the end closes a subordinate clause: *Hat jemand eine Idee, was ich
        // tun kann?*
        if irregular_finite_verb_lemma(&last_word).is_some() {
            return None;
        }
        // *würde ich ins Theater gegangen*: the conditional takes the infinitive. A
        // participle of a verb that forms its perfect with *sein* has no passive
        // (*er würde gelobt* is one), so only those are reported.
        if words
            .iter()
            .any(|word| matches!(word.as_str(), "würde" | "würden" | "würdest" | "würdet"))
            && SEIN_PARTICIPLES.contains(&last_word.as_str())
            && last_word != "geworden"
            && last_word != "gewesen"
        {
            let mut lint = Self::report(
                last,
                "Nach »würde« steht der Infinitiv, nicht das Partizip",
                &last_word,
            );
            lint.suggestions = lemma_of_participle(&last_word)
                .map(|infinitive| vec![Suggestion::ReplaceWith(infinitive.chars().collect())])
                .unwrap_or_default();
            return Some(lint);
        }
        if lemmas
            .iter()
            .flatten()
            .any(|lemma| matches!(*lemma, "werden" | "sein"))
        {
            return None;
        }

        // *Er hat sein Fahrrad repariert lassen*: *lassen* takes the bare infinitive.
        if last_word == "lassen"
            && let Some(&previous_at) = before.last()
            && starts_lowercase(tokens[previous_at], document)
            && is_participle_not_infinitive(&words[words.len() - 1])
        {
            let previous = tokens[previous_at];
            let infinitive = infinitive_of_participle(&words[words.len() - 1]);
            return Some(Lint {
                span: previous.span,
                lint_kind: LintKind::Grammar,
                suggestions: infinitive
                    .map(|w| vec![Suggestion::ReplaceWith(w.chars().collect())])
                    .unwrap_or_default(),
                message: format!(
                    "Vor »lassen« steht der Infinitiv, nicht das Partizip: »{}«.",
                    words[words.len() - 1]
                ),
                priority: 31,
            });
        }

        let modal = lemmas.iter().flatten().any(|lemma| MODALS.contains(lemma));
        let haben = lemmas
            .iter()
            .flatten()
            .filter(|lemma| **lemma == "haben")
            .count()
            == 1;

        if modal && !haben && is_past_participle(&last_word) {
            return Some(Self::report(
                last,
                "Nach einem Modalverb steht der Infinitiv, nicht das Partizip",
                &last_word,
            ));
        }

        let previous = words.last().map(String::as_str).unwrap_or("");
        let infinitive = last_word.ends_with('n')
            && last.kind.is_verb()
            && !last.kind.is_adjective()
            && !is_past_participle(&last_word)
            // *geholfen*: an intransitive participle has no adjective reading.
            && !looks_like_participle(&last_word)
            && !participle_spelled_like_infinitive(&last_word)
            && !has_zu_infix(&last_word)
            && !INFINITIVE_LAST.contains(&last_word.as_str());
        let bare = previous != "zu"
            && !matches!(previous, "gut" | "leicht")
            // *zu viel äußern … Sinn*: a quantifier in front makes the word an
            // attribute.
            && !matches!(previous, "viel" | "wenig" | "mehr")
            // Two infinitives: *hat ihn kommen sehen*, *hat schwimmen gehen*.
            && !(previous.ends_with("en") && tokens[*before.last()?].kind.is_verb());
        // *Nachdem er gegessen hat ging er schlafen*: a second finite verb means the
        // comma is missing and *schlafen* belongs to *ging*, not to *hat*.
        let other_finite = lemmas
            .iter()
            .flatten()
            .any(|lemma| *lemma != "haben" && !MODALS.contains(lemma))
            || before.iter().zip(&words).any(|(&at, word)| {
                tokens[at].kind.is_verb()
                    && starts_lowercase(tokens[at], document)
                    && irregular_finite_verb_lemma(word).is_none()
                    && !word.ends_with('n')
                    && !is_past_participle(word)
                    && !looks_like_participle(word)
            });
        if haben
            && !modal
            && !other_finite
            && infinitive
            && bare
            && !self.is_a_noun_written_small(&last_word, tokens[*before.last()?], document)
        {
            return Some(Self::report(
                last,
                "Das Perfekt mit »haben« verlangt das Partizip, nicht den Infinitiv",
                &last_word,
            ));
        }
        None
    }

    fn report(token: &Token, message: &str, word: &str) -> Lint {
        Lint {
            span: token.span,
            lint_kind: LintKind::Grammar,
            suggestions: Vec::new(),
            message: format!("{message}: »{word}«."),
            priority: 31,
        }
    }
}

impl Linter for GermanVerbCluster {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let tokens: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            for clause in Self::clauses(&tokens, document) {
                if let Some(lint) = self.check(&clause, &tokens, document) {
                    lints.push(lint);
                }
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Prüft die Verbform am Satzende: Infinitiv nach Modalverben (»kann machen«), Partizip im Perfekt (»hat gemacht«)."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanVerbCluster;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn flagged(text: &str) -> Vec<String> {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanVerbCluster::new()
            .lint(&document)
            .into_iter()
            .map(|lint| document.get_span_content_str(&lint.span))
            .collect()
    }

    fn suggested(text: &str) -> Vec<String> {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanVerbCluster::new()
            .lint(&document)
            .into_iter()
            .flat_map(|lint| lint.suggestions)
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn wuerde_takes_the_infinitive() {
        assert_eq!(
            suggested("Wenn ich mehr Zeit hätte, würde ich öfter ins Theater gegangen."),
            ["Replace with: “gehen”"]
        );
        assert_eq!(
            suggested("Ich würde gerne früher gekommen."),
            ["Replace with: “kommen”"]
        );
        for text in [
            "Wenn ich mehr Zeit hätte, würde ich öfter ins Theater gehen.",
            "Er würde gelobt, wenn er pünktlich wäre.",
            "Ich wäre gerne gekommen, wenn ich gekonnt hätte.",
            "Er würde krank geworden sein.",
            "Ich würde nicht gegangen sein.",
            "Nachdem er gegessen hat ging er schlafen.",
            "Wir haben ein Haus gekauft in dem wir wohnen.",
        ] {
            assert!(flagged(text).is_empty(), "{text}: {:?}", flagged(text));
        }
    }

    #[test]
    fn participles_sharing_a_later_auxiliary_are_quiet() {
        for text in [
            "Der Boden kann nicht verkauft, sondern nur vererbt werden.",
            "Das Gebäude muss saniert, aber nicht abgerissen werden.",
            "Die Regel darf geändert, aber nicht aufgehoben werden.",
            "Die Daten können gelesen, aber nicht verändert werden.",
        ] {
            assert!(flagged(text).is_empty(), "{text}: {:?}", flagged(text));
        }
        assert_eq!(
            flagged("Ich kann das nicht gemacht, aber ich versuche es."),
            ["gemacht"]
        );
    }

    #[test]
    fn a_participle_before_lassen_takes_the_infinitive() {
        for (text, infinitive) in [
            ("Er hat sein Fahrrad repariert lassen.", "reparieren"),
            ("Ich habe das Auto gewaschen lassen.", "waschen"),
            ("Sie hat die Haare geschnitten lassen.", "schneiden"),
            ("Wir haben das Paket abgeholt lassen.", "abholen"),
            ("Er hat das Haus verkauft lassen.", "verkaufen"),
        ] {
            let suggestions = suggested(text);
            assert_eq!(flagged(text).len(), 1, "{text}");
            if infinitive != "schneiden" {
                assert!(
                    suggestions.iter().any(|s| s.contains(infinitive)),
                    "{text}: {suggestions:?}"
                );
            }
        }
        for text in [
            "Er hat sein Fahrrad reparieren lassen.",
            "Er hat sich gehen lassen.",
            "Sie hat es geschehen lassen.",
            "Er hat ihn gefesselt liegen lassen.",
            "Wir haben uns scheiden lassen.",
            "Das hat er sich nicht gefallen lassen.",
        ] {
            assert!(flagged(text).is_empty(), "{text}: {:?}", flagged(text));
        }
    }

    #[test]
    fn a_participle_behind_a_modal_is_reported() {
        for (text, word) in [
            ("Man kann die Größe nicht beliebig erhöht.", "erhöht"),
            ("Er will alle Kunden ausfindig gemacht.", "gemacht"),
            ("Wir müssen das heute noch erledigt.", "erledigt"),
            ("Der Preis dürfte bei 50 Mark gelegen.", "gelegen"),
            ("Du sollst das Geld sofort investiert.", "investiert"),
            (
                "Ich kann dir morgen das Geld gegeben, wenn du willst.",
                "gegeben",
            ),
        ] {
            assert_eq!(flagged(text), [word], "{text}");
        }
    }

    #[test]
    fn an_infinitive_behind_haben_is_reported() {
        for (text, word) in [
            ("Intel hat den Termin erneut verschieben.", "verschieben"),
            ("Ich habe gestern das Buch lesen.", "lesen"),
            ("Wir haben den ganzen Abend Karten spielen.", "spielen"),
            (
                "Hat das Opfer das Bild speichern, so kann es passieren.",
                "speichern",
            ),
        ] {
            assert_eq!(flagged(text), [word], "{text}");
        }
    }

    #[test]
    fn a_finite_modal_closing_a_subordinate_clause_is_quiet() {
        assert!(
            flagged("Hat jemand eine Idee was ich tun kann?").is_empty(),
            "kann"
        );
    }

    #[test]
    fn correct_verb_clusters_are_quiet() {
        for text in [
            "Man kann die Größe nicht beliebig erhöhen.",
            "Das kann gemacht werden.",
            "Das muss erledigt sein.",
            "Er soll das gesehen haben.",
            "Ich habe das Buch gelesen.",
            "Er hat das Paket bekommen.",
            "Sie hat ihren Schirm vergessen.",
            "Er hat nicht kommen können.",
            "Sie hat ihn gehen lassen.",
            "Er hat viel Geld auf dem Konto liegen.",
            "Du hast gut reden.",
            "Ich habe noch viel zu tun.",
            "Wir haben beschlossen, morgen zu fahren.",
            "Er hat gesagt, dass er kommen will.",
            "Ich möchte das Fenster geschlossen.",
            "Er will, dass alles erledigt ist.",
            "Sie kann gut kochen und hat viel gelernt.",
            "Er kann es kaum erwarten.",
            "Das Geschäft hat bis acht Uhr offen.",
            "Wir haben Ferien.",
            "Sie hat das Fenster offen.",
            "Das hat zur Folge, dass wir warten müssen.",
            "Wer hat Lust mitzukommen?",
            "Niemand hat mir geholfen.",
            "Wir konnten das Essen genießen.",
            "Man kann davon ausgehen, dass es klappt.",
            "Wir müssen uns noch gedulden.",
            "Die Partei konnte alle Wahlkreise gewinnen.",
            "Das könnte häufiger geschehen.",
            "Ich habe ganz andere sorgen.",
        ] {
            assert!(flagged(text).is_empty(), "{text}: {:?}", flagged(text));
        }
    }
}
