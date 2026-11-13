//! *weshalb ich **Komme***, *der Vortrag war sehr **Interessant***, *sie hat
//! mir **Gestern** geschrieben*: a word that is no noun, written with a capital
//! in the middle of a sentence.

use std::sync::Arc;

use harper_core::{
    Document, Punctuation, Token, TokenKind, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
    spell::Dictionary,
};

use crate::grammar::subjects::subject_pronoun;
use crate::spell::compound_aware_dict::CompoundAwareDictionary;
use crate::spell::compound_aware_german_dictionary;

/// The words that grade an adjective: *sehr*, *ziemlich*, *zu*. What follows
/// one of them is the adjective it grades, never a noun.
const INTENSIFIERS: &[&str] = &[
    "sehr",
    "ziemlich",
    "total",
    "echt",
    "wirklich",
    "extrem",
    "besonders",
    "äußerst",
    "recht",
    "ganz",
    "zu",
    "so",
    "allzu",
    "unglaublich",
    "richtig",
    "super",
    "eher",
    "relativ",
    "voll",
];

/// The adverbs of time that are nouns only behind an article: *das Gestern*,
/// *im Heute*, *am Morgen*.
const DAY_ADVERBS: &[&str] = &["gestern", "heute", "morgen", "vorgestern", "übermorgen"];

/// The adverbs of repeated time, *montags*, *abends*: a noun only as the
/// genitive behind an article (*des Abends*, *eines Montags*).
const RECURRING_ADVERBS: &[&str] = &[
    "montags",
    "dienstags",
    "mittwochs",
    "donnerstags",
    "freitags",
    "samstags",
    "sonntags",
    "morgens",
    "vormittags",
    "mittags",
    "nachmittags",
    "abends",
    "nachts",
    "werktags",
    "feiertags",
    "wochentags",
];

/// Adjectives that end in *-er* in their bare form: *lecker* is not *leck* with an ending.
const ER_ADJECTIVES: &[&str] = &[
    "bitter", "munter", "heiter", "finster", "lecker", "sauber", "sicher", "teuer", "sauer",
    "mager", "locker", "tapfer", "bieder", "wacker", "lauter", "düster", "clever", "super",
];

/// The verbs a predicative adjective follows: *ist schön*, *wird krank*, *schmeckt lecker*.
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
    "werde",
    "wirst",
    "wird",
    "werden",
    "wurde",
    "wurden",
    "bleibt",
    "bleiben",
    "blieb",
    "blieben",
    "wirkt",
    "wirkte",
    "scheint",
    "schien",
    "schmeckt",
    "schmeckte",
    "klingt",
    "klang",
    "aussieht",
    "riecht",
    "fühlt",
    "fühle",
];

/// *Ich finde das toll.*: the forms of *finden* that take a predicative adjective after *das*.
const FINDEN: &[&str] = &["finde", "findest", "findet", "finden", "fand", "fanden"];

/// Adjectives whose capitalized spelling is also a noun so rare that the capital is a slip:
/// *der Lecker* is the hunter's word for a tongue.
const RARE_NOUNS_OF_ADJECTIVES: &[&str] = &["lecker"];

/// What follows *bis* in a farewell: *bis bald*, *bis später*, *bis morgen*.
const AFTER_BIS: &[&str] = &[
    "bald",
    "später",
    "gleich",
    "dann",
    "morgen",
    "nachher",
    "demnächst",
    "übermorgen",
    "nächste",
];

/// Reports a capitalized word in mid-sentence where nothing can make it a noun.
///
/// German capitalizes nouns, and almost any word can become one: *das Gute*,
/// *etwas Neues*, *auf Deutsch*, *Lesen macht Spaß*. Read over a wide stretch
/// of encyclopedic prose, a rule that reported every capitalized non-noun
/// reported mostly nouns the dictionary did not know (*Übergriffe*,
/// *Festrede*), names (*Max Frisch*, *University Press*), headings and the
/// imperatives of grammar books (*Rechne!*, *Atme!*). So this one only looks
/// where the word in front leaves no doubt:
///
/// * after a word that grades an adjective — *sehr Interessant*, *zu
///   Teuer* — when the adjective ends the phrase;
/// * right after a subject pronoun, for a finite verb — *weil ich Komme*,
///   *ob du Kommst*;
/// * for *gestern*, *heute* and *morgen*, after a verb or a pronoun — *sie
///   hat mir Gestern geschrieben*, *ich gehe Morgen zum Arzt*.
///
/// The capitalized spelling must not be a noun, a compound noun or a name in
/// the dictionary, apart from the three adverbs of time, which have their own
/// noun (*der Morgen*) and are taken by context alone.
pub struct GermanMidSentenceCapital {
    /// The compound-aware dictionary: *Ausdrücke* and *Festrede* are nouns
    /// only as compounds.
    dictionary: Arc<CompoundAwareDictionary>,
}

impl Default for GermanMidSentenceCapital {
    fn default() -> Self {
        Self {
            dictionary: compound_aware_german_dictionary(),
        }
    }
}

fn is_capitalized(word: &[char]) -> bool {
    word.len() >= 3 && word[0].is_uppercase() && word[1..].iter().all(|c| c.is_lowercase())
}

impl GermanMidSentenceCapital {
    /// Is the capitalized `word` no noun and no name, and its small spelling a
    /// word of the kind `wanted` accepts?
    fn small_word(
        &self,
        word: &[char],
        wanted: impl Fn(&harper_core::DictWordMetadata) -> bool,
    ) -> bool {
        // The capital as written, a compound included, or a noun reading of the
        // word in any casing: then the capital may be that noun.
        if self.dictionary.contains_exact_word(word)
            || self
                .dictionary
                .get_word_metadata(word)
                .is_some_and(|m| m.is_noun() || m.is_proper_noun())
        {
            return false;
        }
        let lower: Vec<char> = word.iter().flat_map(|c| c.to_lowercase()).collect();
        self.dictionary
            .get_word_metadata(&lower)
            .is_some_and(|metadata| wanted(&metadata))
    }

    fn lint_at(&self, words: &[&Token], at: usize, document: &Document) -> Option<Lint> {
        let token = words[at];
        let chars = document.get_span_content(&token.span);
        if !token.kind.is_word() || !is_capitalized(chars) {
            return None;
        }
        let previous = words.get(at.checked_sub(1)?)?;
        if !previous.kind.is_word() {
            return None;
        }
        let source = document.get_source();
        if source[previous.span.end..token.span.start].contains(&'\n')
            || source.get(token.span.end) == Some(&'-')
        {
            return None;
        }
        let previous_word = document.get_span_content_str(&previous.span);
        // *das Spiel Knapp verloren*: a noun with its article in front, and a verb behind.
        let object_in_front = at > 2
            && words[at - 2].kind.is_determiner()
            && previous.kind.is_noun()
            && words.get(at + 1).is_some_and(|t| {
                t.kind.is_verb()
                    && document
                        .get_span_content(&t.span)
                        .first()
                        .is_some_and(|c| c.is_lowercase())
            });
        if previous_word.chars().next().is_some_and(char::is_uppercase)
            && at > 1
            && !object_in_front
        {
            return None;
        }
        let previous_lower = previous_word.to_lowercase();
        let lower: String = chars.iter().flat_map(|c| c.to_lowercase()).collect();
        let next = words.get(at + 1);
        let next_is_capitalized = next.is_some_and(|t| {
            t.kind.is_word()
                && document
                    .get_span_content(&t.span)
                    .first()
                    .is_some_and(|c| c.is_uppercase())
        });

        // A predicate adjective is bare. With an ending it is a noun: *zu
        // Psychischen*, *so Großes*.
        let bare = !["e", "en", "em", "es"]
            .iter()
            .any(|ending| lower.ends_with(ending));
        // Behind *sehr* even a word with a noun of the same spelling is the adjective: *sehr
        // Gut* is not *das Gut*. Behind *zu* it may be the noun: *zu Recht*.
        let adjective_reading =
            |m: &harper_core::DictWordMetadata| m.adjective.is_some() && m.verb.is_none();
        let graded_adjective = INTENSIFIERS.contains(&previous_lower.as_str())
            && bare
            && !lower.ends_with("sch")
            && if previous_lower == "zu" {
                self.small_word(chars, adjective_reading)
            } else {
                self.dictionary
                    .get_word_metadata(&lower.chars().collect::<Vec<_>>())
                    .is_some_and(|m| adjective_reading(&m))
            };
        // *Das Wetter ist heute Schön.*, *Er hat das Spiel Knapp verloren.*: a bare adjective
        // behind a copula or an adverb, at the end of the clause or before the participle that
        // ends it, is predicative or adverbial, and those are never capitalized.
        let ends_clause = |t: Option<&&Token>| {
            t.is_none_or(|t| {
                matches!(
                    t.kind,
                    TokenKind::Punctuation(
                        Punctuation::Period
                            | Punctuation::Comma
                            | Punctuation::Bang
                            | Punctuation::Question
                    )
                )
            })
        };
        let next_ends_clause = ends_clause(next);
        let verb_ends_clause = next.is_some_and(|t| {
            t.kind.is_verb()
                && document
                    .get_span_content(&t.span)
                    .first()
                    .is_some_and(|c| c.is_lowercase())
        }) && ends_clause(words.get(at + 2));
        let behind_copula = COPULAS.contains(&previous_lower.as_str())
            || (previous.kind.is_adverb()
                && !previous.kind.is_noun()
                && !previous.kind.is_preposition()
                && !matches!(
                    previous_lower.as_str(),
                    "wie" | "als" | "namens" | "genannt"
                )
                && at > 1
                && words[at - 2].kind.is_verb())
            || (previous_lower == "das"
                && at > 1
                && FINDEN.contains(
                    &document
                        .get_span_content_str(&words[at - 2].span)
                        .to_lowercase()
                        .as_str(),
                ));
        let uninflected = match lower.strip_suffix("er") {
            Some(stem) => {
                ER_ADJECTIVES.contains(&lower.as_str())
                    || !lower.ends_with("ter")
                        && !self
                            .dictionary
                            .get_word_metadata(&stem.chars().collect::<Vec<_>>())
                            .is_some_and(|m| m.adjective.is_some())
            }
            None => true,
        };
        let bare_adjective = bare
            && uninflected
            && at > 1
            && ((next_ends_clause && behind_copula)
                || (verb_ends_clause
                    && (previous_word.chars().all(char::is_lowercase) || object_in_front)
                    && !previous.kind.is_determiner()
                    && !previous.kind.is_preposition()
                    && !previous.kind.is_conjunction()))
            && (self.small_word(chars, |m| {
                m.adjective.is_some() && m.verb.is_none() && !m.is_determiner()
            }) || RARE_NOUNS_OF_ADJECTIVES.contains(&lower.as_str()));
        // *Wir gehen heute Schwimmen.*: the infinitive of what one goes to do.
        let goes_to_do = lower.ends_with("en")
            && next_ends_clause
            && !previous.kind.is_determiner()
            && !previous.kind.is_preposition()
            && words[..at].iter().rev().take(3).any(|t| {
                let w = document.get_span_content_str(&t.span).to_lowercase();
                matches!(
                    w.as_str(),
                    "gehe" | "gehst" | "geht" | "gehen" | "ging" | "gingen" | "gingst" | "gegangen"
                )
            })
            && self
                .dictionary
                .get_word_metadata(&lower.chars().collect::<Vec<_>>())
                .is_some_and(|m| m.is_verb());
        let finite_verb = subject_pronoun(&previous_lower).is_some()
            && !lower.ends_with('n')
            && self.small_word(chars, |m| m.verb.is_some() && m.adjective.is_none());
        let day_adverb = DAY_ADVERBS.contains(&lower.as_str())
            && (previous.kind.is_verb() || previous.kind.is_pronoun())
            && !previous.kind.is_determiner()
            && !next_is_capitalized
            && !matches!(
                previous_lower.as_str(),
                "guten" | "jeden" | "diesen" | "den" | "einen" | "am" | "vom" | "zum"
            );
        let recurring = RECURRING_ADVERBS.contains(&lower.as_str())
            && !matches!(
                previous_lower.as_str(),
                "des" | "eines" | "jedes" | "dieses" | "jenes"
            );
        let farewell = previous_lower == "bis" && AFTER_BIS.contains(&lower.as_str());
        // *Wir haben Zwölf Gäste eingeladen*, *Er ist Dreißig geworden*: a numeral in
        // mid-sentence before a noun or *werden* is no noun; with an article it is (*die Zwölf*).
        let numeral = crate::spell::lexical_classes::NUMERALS.contains(&lower)
            && !lower.starts_with("ein")
            // *Millionen*, *Milliarden*, *Dutzend* are nouns.
            && !["ion", "ionen", "iarde", "iarden", "dutzend", "dutzende"]
                .iter()
                .any(|ending| lower.ends_with(ending))
            && !previous.kind.is_conjunction()
            && !previous.kind.is_determiner()
            && !previous.kind.is_preposition()
            && next.is_some_and(|t| {
                (t.kind.is_noun() && next_is_capitalized)
                    || matches!(
                        crate::grammar::noun_phrase::lowercase_of(t, document).as_str(),
                        "geworden" | "werden" | "wird" | "jahre" | "alt"
                    )
            });
        // *Die Beiden sind verheiratet*: *beide* behind the article, with no noun of its own.
        let beide = matches!(lower.as_str(), "beiden" | "beide")
            && previous.kind.is_determiner()
            && !next_is_capitalized;
        if !(graded_adjective
            || bare_adjective
            || goes_to_do
            || finite_verb
            || day_adverb
            || recurring
            || farewell
            || numeral
            || beide)
        {
            return None;
        }

        Some(Lint {
            span: token.span,
            lint_kind: LintKind::Capitalization,
            suggestions: vec![Suggestion::ReplaceWith(lower.chars().collect())],
            message: format!("»{lower}« ist hier kein Nomen und wird kleingeschrieben."),
            priority: 26,
        })
    }
}

impl Linter for GermanMidSentenceCapital {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            lints.extend((1..words.len()).filter_map(|at| self.lint_at(&words, at, document)));
        }
        lints
    }

    fn description(&self) -> &str {
        "Schreibt Verben, Adverbien und Adjektive klein, die mitten im Satz großgeschrieben sind."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanMidSentenceCapital;
    use crate::parsers::PlainGerman;
    use crate::spell::compound_aware_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn flagged(text: &str) -> Vec<String> {
        let dictionary = compound_aware_german_dictionary();
        let document = Document::new(text, &PlainGerman, &*dictionary);
        GermanMidSentenceCapital::default()
            .lint(&document)
            .iter()
            .map(|lint| document.get_span_content_str(&lint.span))
            .collect()
    }

    #[test]
    fn catches_a_capitalized_verb_adverb_or_adjective() {
        for (text, word) in [
            ("Das ist der Grund, weshalb ich Komme.", "Komme"),
            ("Der Vortrag war sehr Interessant.", "Interessant"),
            ("Das ist mir zu Teuer.", "Teuer"),
            ("Sie hat mir Gestern geschrieben.", "Gestern"),
            ("Ich gehe Morgen zum Arzt.", "Morgen"),
            ("Ob du Kommst, weiß ich nicht.", "Kommst"),
            ("Wir treffen uns Montags.", "Montags"),
            ("Sie arbeitet Abends lange.", "Abends"),
            ("Bis Bald!", "Bald"),
            ("Viele Grüße und bis Morgen.", "Morgen"),
            ("Er spielt sehr Gut Klavier.", "Gut"),
            ("Das Wetter ist heute Schön.", "Schön"),
            ("Er ist heute Krank.", "Krank"),
            ("Das Essen schmeckt Lecker.", "Lecker"),
            ("Er antwortet immer Höflich.", "Höflich"),
            ("Er hat das Spiel Knapp verloren.", "Knapp"),
            ("Ich finde das Toll.", "Toll"),
            ("Die Produkte sind Top.", "Top"),
            ("Wir gehen heute Schwimmen.", "Schwimmen"),
            ("Sie geht Tanzen.", "Tanzen"),
            ("Wir haben Zwölf Gäste eingeladen.", "Zwölf"),
            ("Er ist Dreißig geworden.", "Dreißig"),
            ("Die Beiden sind verheiratet.", "Beiden"),
        ] {
            assert_eq!(flagged(text), vec![word], "{text}");
        }
    }

    #[test]
    fn leaves_nominalizations_and_names_alone() {
        for text in [
            "Ich wünsche dir alles Gute.",
            "Er sagt etwas Interessantes.",
            "Das Gestern ist vorbei.",
            "Wir fahren mit der Deutschen Bahn.",
            "Sie spricht gut Deutsch.",
            "Ich gehe am Morgen joggen.",
            "Guten Morgen, sagte er.",
            "Kommen Sie bitte herein.",
            "Ich danke Ihnen für die Hilfe.",
            "Er sagte: Komm her.",
            "Wir haben Gutes getan.",
            "Das Rote Kreuz hilft.",
            "Er ist sehr Deutsch geprägt.",
            "Es ist ganz Ausdrücke wert.",
            "Er war ganz Ohr.",
            "Wir Deutschen essen gern.",
            "Laut Max Frisch gibt es mehr.",
            "Eines Abends kam er.",
            "Er kam des Nachts.",
            "Ich habe Hunger.",
            "Das Verhältnis des Physischen zu Psychischen.",
            "Er geht zum Schwimmen.",
            "Sie wohnt in der Firma Schön.",
            "Er sagt nichts Neues.",
            "Er war ganz Ohr und hörte zu.",
            "Das Gasthaus heißt Alt Berlin.",
            "Wir gehen das Problem an.",
            "Er spricht gut Englisch.",
            "Das hat er zu Recht gesagt.",
            "Die Zwölf ist meine Glückszahl.",
            "Die beiden Kinder spielen.",
            "Er würfelte eine Sechs.",
            "Das sagt Stark.",
            "Außerdem wurde Ehrlich mit Unterstützung bekannt.",
            "Dort trifft Rosa wieder auf Latimer.",
            "Er wurde Zwölfter in der Meisterschaft.",
            "Er wurde jeweils Zehnter.",
            "Von 1867 bis 1868 war er Beauftragter für Indianerfragen.",
            "Er wurde Bayerischer Meister.",
            "Er spricht Englisch.",
        ] {
            assert!(flagged(text).is_empty(), "{text}: {:?}", flagged(text));
        }
    }
}
