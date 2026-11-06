//! *weshalb ich **Komme***, *der Vortrag war sehr **Interessant***, *sie hat
//! mir **Gestern** geschrieben*: a word that is no noun, written with a capital
//! in the middle of a sentence.

use std::sync::Arc;

use harper_core::{
    Document, Token, TokenStringExt,
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
        if previous_word.chars().next().is_some_and(char::is_uppercase) && at > 1 {
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
        let graded_adjective = INTENSIFIERS.contains(&previous_lower.as_str())
            && bare
            && !next_is_capitalized
            && self.small_word(chars, |m| m.adjective.is_some() && m.verb.is_none());
        let finite_verb = subject_pronoun(&previous_lower).is_some()
            && !lower.ends_with('n')
            && self.small_word(chars, |m| m.verb.is_some() && m.adjective.is_none());
        let day_adverb = DAY_ADVERBS.contains(&lower.as_str())
            && (previous.kind.is_verb() || previous.kind.is_pronoun())
            && !previous.kind.is_determiner()
            && !next_is_capitalized
            && !matches!(
                previous_lower.as_str(),
                "guten" | "jeden" | "diesen" | "den" | "einen" | "am" | "vom" | "zum" | "bis"
            );
        if !(graded_adjective || finite_verb || day_adverb) {
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
            "Sie wartete bis Morgen früh.",
            "Ich habe Hunger.",
            "Das Verhältnis des Physischen zu Psychischen.",
        ] {
            assert!(flagged(text).is_empty(), "{text}: {:?}", flagged(text));
        }
    }
}
