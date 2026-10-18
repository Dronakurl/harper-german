use harper_core::{
    AdjectiveData, DictWordMetadata, Document, Token, TokenKind, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
    spell::Dictionary,
};

/// Reports an adjective that only exists as a compound and is written with a
/// capital in the middle of a sentence: *Das ist Skistiefelnd*.
///
/// A compound that has no entry of its own gets its word class from its last
/// element (see `CompoundChecker::get_compound_metadata`), and that metadata is
/// all there is to it: an adjective and nothing else, not even the spelling
/// flags a dictionary entry carries. That is what this rule looks for, so it
/// leaves every word the word list knows alone, surnames such as *Klein* and
/// *Groß* among them.
///
/// Only compounds that end in a present participle are looked at. The
/// metadata above is also what the splitter hands a noun whose first element
/// looks like an adjective (*Kurzgeschichten*, *Weißwein*, *Sicherungskonzept*),
/// and for those the capital is right. A participle head is told apart by the
/// dictionary: the word minus its *-d* must end in a known verb (*Ski|stiefel|n*
/// ends in *stiefeln*), which *Abend*, *Trend* and *Jugend* do not.
///
/// The capital is only a mistake where nothing licenses a noun phrase. After a
/// determiner, a preposition or another adjective it may be a nominalization
/// (*das Skistiefelnde*, *im Skistiefelnden*), and behind a capitalized word it
/// may be part of a name (*Herr Hellmich*), so those are left alone, and so is
/// the start of a sentence. Only a finite verb or an adverb in front counts: *ist Skistiefelnd*, *sehr Skistiefelnd*.
///
/// The lower-case spelling is not reported: it is a legal compound by the
/// splitter's rules, and narrowing those is a separate and measured trade-off
/// (see the notes on the over-permissive splitter).
pub struct GermanCompoundAdjectiveCapitalization<T: Dictionary> {
    dictionary: T,
}

impl<T: Dictionary> GermanCompoundAdjectiveCapitalization<T> {
    pub fn new(dictionary: T) -> Self {
        Self { dictionary }
    }

    /// Does `lower` end in a present participle: a known verb plus *-d*?
    fn ends_in_present_participle(&self, lower: &[char]) -> bool {
        let Some(infinitive) = lower.strip_suffix(&['d']) else {
            return false;
        };
        if !(infinitive.ends_with(&['e', 'n'])
            || infinitive.ends_with(&['e', 'l', 'n'])
            || infinitive.ends_with(&['e', 'r', 'n']))
        {
            return false;
        }
        // At least three letters in front and four in the verb.
        (3..infinitive.len().saturating_sub(3)).any(|split| {
            self.dictionary
                .get_word_metadata(&infinitive[split..])
                .is_some_and(|m| m.verb.is_some())
        })
    }
}

fn synthesized_adjective() -> DictWordMetadata {
    DictWordMetadata {
        adjective: Some(AdjectiveData::default()),
        ..Default::default()
    }
}

impl<T: Dictionary> Linter for GermanCompoundAdjectiveCapitalization<T> {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let synthesized = synthesized_adjective();
        let mut lints = Vec::new();

        for chunk in document.iter_chunks() {
            let tokens: Vec<&Token> = chunk.iter().filter(|t| !t.kind.is_whitespace()).collect();
            for window in tokens.windows(2) {
                let [previous, token] = window else { continue };
                let TokenKind::Word(Some(metadata)) = &token.kind else {
                    continue;
                };
                if *metadata != synthesized {
                    continue;
                }

                let chars = document.get_span_content(&token.span);
                let mut letters = chars.iter();
                let capitalized = letters.next().is_some_and(|c| c.is_uppercase())
                    && letters.all(|c| c.is_lowercase());
                if !capitalized {
                    continue;
                }

                let lower: Vec<char> = chars.iter().flat_map(|c| c.to_lowercase()).collect();
                if !self.ends_in_present_participle(&lower) {
                    continue;
                }

                let previous_chars = document.get_span_content(&previous.span);
                let behind_lowercase_word = matches!(previous.kind, TokenKind::Word(_))
                    && previous_chars.first().is_some_and(|c| c.is_lowercase());
                let kind = &previous.kind;
                let licenses_noun_phrase =
                    kind.is_determiner() || kind.is_preposition() || kind.is_adjective();
                if !behind_lowercase_word
                    || licenses_noun_phrase
                    || !(kind.is_verb() || kind.is_adverb())
                {
                    continue;
                }

                let word: String = chars.iter().collect();
                lints.push(Lint {
                    span: token.span,
                    lint_kind: LintKind::Capitalization,
                    suggestions: vec![Suggestion::ReplaceWith(lower)],
                    message: format!(
                        "»{word}« ist ein zusammengesetztes Adjektiv und wird kleingeschrieben."
                    ),
                    priority: 63,
                });
            }
        }

        lints
    }

    fn description(&self) -> &str {
        "Findet zusammengesetzte Adjektive, die mitten im Satz großgeschrieben sind."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanCompoundAdjectiveCapitalization;
    use crate::parsers::PlainGerman;
    use crate::spell::compound_aware_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn lint_count(text: &str) -> usize {
        let dict = compound_aware_german_dictionary();
        let document = Document::new(text, &PlainGerman, &*dict);
        GermanCompoundAdjectiveCapitalization::new(dict.clone())
            .lint(&document)
            .len()
    }

    #[test]
    fn flags_a_capitalized_compound_adjective_mid_sentence() {
        for text in [
            "Das ist Skistiefelnd.",
            "Der Schuh ist sehr Skistiefelnd.",
            "Sie wirkt Skistiefelnd und müde.",
        ] {
            assert_eq!(lint_count(text), 1, "should fire on {text:?}");
        }
    }

    #[test]
    fn leaves_other_spellings_and_known_words_alone() {
        for text in [
            // lower case, sentence start, nominalization, name
            "Das ist skistiefelnd.",
            "Skistiefelnd ist das nicht.",
            "Das Skistiefelnde ist neu.",
            "Er spricht vom Skistiefelnden.",
            "Herr Hellmich kommt morgen.",
            // words the dictionary has an entry for
            "Das sagt Klein.",
            "Das ist Groß geschrieben.",
            "Das ist Wasserabweisend.",
            // compound nouns the splitter takes for adjectives
            "Wir entwickeln Sicherungskonzepte.",
            "Sie schreibt Kurzgeschichten.",
            "Sie liebt Weißwein.",
            "Wir sehen den Wochenabend.",
            "Er hasst Trend.",
        ] {
            assert_eq!(lint_count(text), 0, "should not fire on {text:?}");
        }
    }
}
