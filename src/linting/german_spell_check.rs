use std::sync::LazyLock;

use hashbrown::{HashMap, HashSet};

use crate::linting::german_foreign_stretch;
use crate::linting::german_relative_clause_comma::GermanRelativeClauseComma;
use crate::spell::compound_checker::{
    MIN_COMPOUND_PART_LEN, can_head_a_capitalized_compound, can_head_a_lowercase_compound,
    has_content_reading, interfix_fits, interfix_matches_known_begin, is_derivational_suffix,
    lowercase, may_be_compound_element, may_open_a_compound, may_take_an_interfix,
};
use crate::spell::german_dict::{
    GERMAN_BARRED_ELEMENTS, GERMAN_FUNCTION_WORDS, GERMAN_STEMS, GERMAN_SUFFIXED_ELEMENTS,
};
use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::spell::{
    Dictionary, EditKind, SuggestionHints, suggest_correct_spelling_with_hints,
};
use harper_core::{Document, Punctuation, Token, TokenKind, TokenStringExt};

// `MIN_COMPOUND_PART_LEN` is imported rather than redeclared: this decomposition
// and the dictionary's own must not disagree about what counts as an element.
const MAX_COMPOUND_PARTS: usize = 5;
const EMPTY_INTERFIX: &[char] = &[];
const S_INTERFIX: &[char] = &['s'];
const N_INTERFIX: &[char] = &['n'];
const EN_INTERFIX: &[char] = &['e', 'n'];
const ER_INTERFIX: &[char] = &['e', 'r'];
const ES_INTERFIX: &[char] = &['e', 's'];
const GERMAN_COMPOUND_INTERFIXES: &[&[char]] = &[
    EMPTY_INTERFIX,
    S_INTERFIX,
    N_INTERFIX,
    EN_INTERFIX,
    ER_INTERFIX,
    ES_INTERFIX,
];

/// How German misspellings come about, read once from the `suggestions`
/// section of `annotations.json`: the alternations writers swap (`ae`/`ä`,
/// `ss`/`ß`, `i`/`ie`, the silent `h`, `ph`), the QWERTZ keyboard, and letter
/// frequency. Ranking by them is the core's
/// [`suggest_correct_spelling_with_hints`]; the table is German data.
static GERMAN_HINTS: LazyLock<SuggestionHints> = LazyLock::new(|| {
    SuggestionHints::from_annotations_json(include_str!("../../data/annotations.json"))
        .expect("the German annotations parse")
});

/// Do the two words start with the same letter, ignoring case?
fn keeps_first_letter(misspelled: &[char], candidate: &[char]) -> bool {
    match (misspelled.first(), candidate.first()) {
        (Some(a), Some(b)) => a.to_lowercase().eq(b.to_lowercase()),
        _ => false,
    }
}

/// The longest run of letters still read as an initialism rather than a word.
///
/// Five covers the abbreviations German prose actually uses unspaced — ISBN,
/// GmbH, DDR, SPD, ZDF, UNHCR — while leaving a genuinely misspelled word set
/// in capitals ("HAUTPBAHNHOF") to the spell checker.
const MAX_INITIALISM_LEN: usize = 5;

/// The longest token a word-internal capital is allowed to mark as an
/// abbreviation. Beyond this a stray capital is more likely a typo in a real
/// compound than an acronym.
const MAX_ABBREVIATION_LEN: usize = 6;

/// Characters a Roman numeral is built from.
const ROMAN_DIGITS: &[char] = &['I', 'V', 'X', 'L', 'C', 'D', 'M'];

/// Returns true for tokens that are not German words at all, and so have no
/// business being measured against a German dictionary.
///
/// Encyclopedic prose is full of these — "Ludwig XIV.", "S. 11", "ISBN", "5 m",
/// "der FC Bayern" — and every one of them was previously reported as a
/// misspelling, usually with an absurd suggestion ("II" → "in"). They account
/// for a large share of the spelling lints on the German Wikipedia corpus.
fn is_non_lexical_token(word: &[char]) -> bool {
    // A single letter is an initial, a variable, or a unit ("A. Müller", "5 m").
    if word.len() == 1 && word[0].is_alphabetic() {
        return true;
    }

    // Anything with a digit in it is an identifier, not a word ("O2", "S12").
    if word.iter().any(char::is_ascii_digit) {
        return true;
    }

    // Roman numerals: regnal numbers, centuries, volume numbers. Checked before
    // the length cap so "XVIII" and "MCMLXXXIV" are covered too.
    if word.iter().all(|c| ROMAN_DIGITS.contains(c)) {
        return true;
    }

    // Initialisms and acronyms.
    if word.len() <= MAX_INITIALISM_LEN && word.iter().all(|c| c.is_uppercase()) {
        return true;
    }

    // A capital inside a short token marks it as an abbreviation, a legal form
    // or a unit symbol rather than a German word: "gGmbH", "UdSSR", "RoHS",
    // "kV", "dB", "mA", "CaO". German orthography has no word-internal capital,
    // so nothing is lost by declining to spell-check these — and every one of
    // them otherwise draws a suggestion list of pure noise.
    word.len() <= MAX_ABBREVIATION_LEN && word[1..].iter().any(|c| c.is_uppercase())
}

/// A spell checker for German text with compound word handling.
pub struct GermanSpellCheck<T>
where
    T: Dictionary,
{
    dictionary: T,
}

impl<T: Dictionary> GermanSpellCheck<T> {
    /// Is `word`, with a full stop after it, an abbreviation — on its own
    /// (*Nr.*, *Okt.*) or at the end of a compound (*Bahnhofstr.*,
    /// *Bestellnr.*)?
    fn is_abbreviation_with_stop(&self, word: &[char]) -> bool {
        // Matched in the entry's own case: the abbreviation of a noun is
        // capitalized (*Prof.*, not *prof.*). A lower-case entry may still
        // open a sentence (*Bzw.*).
        let with_stop = |part: &[char]| {
            let mut chars = part.to_vec();
            chars.push('.');
            if self.dictionary.contains_exact_word(&chars) {
                return true;
            }
            let mut lowered = chars.clone();
            match lowered.first_mut() {
                Some(first) if first.is_uppercase() => {
                    *first = first.to_lowercase().next().unwrap_or(*first);
                    self.dictionary.contains_exact_word(&lowered)
                }
                _ => false,
            }
        };
        with_stop(word)
            || (2..=4).any(|len| {
                word.len() >= len + 3 && {
                    let (head, tail) = word.split_at(word.len() - len);
                    let lower: Vec<char> = tail.iter().flat_map(|c| c.to_lowercase()).collect();
                    // The abbreviation of a noun is entered capitalized
                    // (`Str.`), and the compound writes it lower case.
                    let capitalized: Vec<char> = lower
                        .iter()
                        .enumerate()
                        .flat_map(|(at, c)| {
                            if at == 0 {
                                c.to_uppercase().collect::<Vec<_>>()
                            } else {
                                vec![*c]
                            }
                        })
                        .collect();
                    (with_stop(&lower) || with_stop(&capitalized))
                        && (self.dictionary.contains_word(head)
                            || self.try_compound_word_check(head))
                }
            })
    }

    /// Is `word` a verb or a conjunction with `'s` (*es*) attached: *geht's*,
    /// *war's*, *sag's*, *wenn's*? Only these hosts take the clitic; an
    /// English-style genitive (*Peter's*) is not accepted.
    fn is_cliticized_es(&self, word: &[char]) -> bool {
        const HOSTS: &[&str] = &[
            "wie", "wenn", "ob", "so", "da", "was", "als", "weil", "dass",
        ];
        let [host @ .., apostrophe, 's'] = word else {
            return false;
        };
        // Without the apostrophe the Duden allows it too (*gibts*), but the
        // same shape is the most common misspelling of a second person
        // (*du machts*, *läufts du*) and of *stets* (*stehts*), so only the
        // apostrophe is accepted.
        if !matches!(apostrophe, '\'' | '’') {
            return false;
        }
        if host.is_empty() {
            return false;
        }
        let lower: String = host.iter().flat_map(|c| c.to_lowercase()).collect();
        HOSTS.contains(&lower.as_str())
            || self
                .dictionary
                .get_word_metadata(host)
                .is_some_and(|metadata| metadata.is_verb())
            // The finite form (*geht*) or the imperative (*sag*) of a verb
            // whose form the dictionary carries no reading for.
            || GermanRelativeClauseComma::infinitive_is_a_verb(&lower)
            || GermanRelativeClauseComma::infinitive_is_a_verb(&format!("{lower}t"))
    }

    pub fn new(dictionary: T) -> Self {
        Self { dictionary }
    }

    /// Take `interfix` off the front of `remainder`, if it may stand there.
    ///
    /// `element` is what precedes the seam, because an interfix may not repeat
    /// the letter it follows — see [`interfix_fits`].
    fn strip_compound_interfix<'a>(
        &self,
        element: &[char],
        remainder: &'a [char],
        interfix: &[char],
    ) -> Option<&'a [char]> {
        if !interfix_fits(element, interfix)
            || !interfix_matches_known_begin(element, interfix)
            || (!interfix.is_empty() && !may_take_an_interfix(&self.dictionary, element))
        {
            return None;
        }
        // A derived feminine noun takes `-s-` and nothing else; see
        // `suffixed_element_set`.
        if interfix != S_INTERFIX && GERMAN_SUFFIXED_ELEMENTS.contains(element) {
            return None;
        }
        let rest = remainder.strip_prefix(interfix)?;
        // And nothing at all stands in front of the suffix itself.
        if !interfix.is_empty() && is_derivational_suffix(rest) {
            return None;
        }
        Some(rest)
    }

    /// May this element end a compound? See `function_word_set`.
    fn may_close_a_compound(&self, element: &[char]) -> bool {
        self.is_element(element)
            && (!GERMAN_FUNCTION_WORDS.contains(element)
                || has_content_reading(&self.dictionary, element))
    }

    /// May this element take part in a compound at all? See
    /// `may_be_compound_element`.
    fn is_element(&self, element: &[char]) -> bool {
        may_be_compound_element(&self.dictionary, &GERMAN_BARRED_ELEMENTS, element)
    }

    fn is_valid_compound_segment(
        &self,
        word: &[char],
        depth: usize,
        memo: &mut HashMap<Vec<char>, bool>,
        lowercase_whole: Option<&[char]>,
    ) -> bool {
        if word.len() < MIN_COMPOUND_PART_LEN {
            return false;
        }

        if depth >= MAX_COMPOUND_PARTS {
            return false;
        }

        // The whole remaining tail is one element, so this is the compound's
        // last element. See `can_head_a_lowercase_compound` and
        // `function_word_set`.
        if depth > 0 && self.dictionary.contains_word(word) && self.may_close_a_compound(word) {
            return match lowercase_whole {
                Some(whole) => can_head_a_lowercase_compound(&self.dictionary, whole, word),
                None => can_head_a_capitalized_compound(&self.dictionary, word),
            };
        }

        if let Some(cached) = memo.get(word) {
            return *cached;
        }

        let mut valid = false;

        for split_pos in MIN_COMPOUND_PART_LEN..=word.len() - MIN_COMPOUND_PART_LEN {
            let first_part = &word[..split_pos];

            // A bare verb stem opens a compound without being a word:
            // `Messkabel` is `mess` plus `Kabel`, and `mess` alone is not
            // German. It may not *end* one, which is why this is the only
            // place the stems are consulted.
            if !self.dictionary.contains_word(first_part) && !GERMAN_STEMS.contains(first_part) {
                continue;
            }

            // A function word may open a compound but not stand inside one;
            // see `may_close_a_compound`.
            if !self.is_element(first_part)
                || !may_open_a_compound(&self.dictionary, &GERMAN_STEMS, first_part)
                || (depth > 0 && !self.may_close_a_compound(first_part))
            {
                continue;
            }

            let remainder = &word[split_pos..];

            for interfix in GERMAN_COMPOUND_INTERFIXES {
                let Some(next_part) = self.strip_compound_interfix(first_part, remainder, interfix)
                else {
                    continue;
                };

                if next_part.len() < MIN_COMPOUND_PART_LEN {
                    continue;
                }

                // In German, compound noun parts are capitalized. Try both the original
                // and capitalized versions of the next part.
                let mut capitalized_next_part = next_part.to_vec();
                if let Some(first_char) = capitalized_next_part.first_mut() {
                    *first_char = first_char.to_uppercase().next().unwrap_or(*first_char);
                }

                let ends_here = |part: &[char]| {
                    self.dictionary.contains_word(part)
                        && self.may_close_a_compound(part)
                        && match lowercase_whole {
                            Some(whole) => {
                                can_head_a_lowercase_compound(&self.dictionary, whole, part)
                            }
                            None => can_head_a_capitalized_compound(&self.dictionary, part),
                        }
                };

                if ends_here(next_part)
                    || ends_here(&capitalized_next_part)
                    || self.is_valid_compound_segment(next_part, depth + 1, memo, lowercase_whole)
                    || self.is_valid_compound_segment(
                        &capitalized_next_part,
                        depth + 1,
                        memo,
                        lowercase_whole,
                    )
                {
                    valid = true;
                    break;
                }
            }

            if valid {
                break;
            }
        }

        memo.insert(word.to_vec(), valid);
        valid
    }

    /// Check if a word is a valid German compound.
    /// German freely combines nouns and often inserts linking morphemes such as
    /// `s`, `n`, `en`, `er`, `e`, or `es` between parts.
    fn try_compound_word_check(&self, word: &[char]) -> bool {
        if word.len() < MIN_COMPOUND_PART_LEN * 2 {
            return false;
        }

        let mut memo = HashMap::new();
        self.is_valid_compound_segment(word, 0, &mut memo, lowercase(word).then_some(word))
    }

    /// Get spelling suggestions for a word.
    ///
    /// The core ranks the candidates with [`GERMAN_HINTS`]: by the kind of
    /// mistake first (an alternation, a neighbouring key, a transposition, a
    /// letter too many or too few, a substitution), so that `Worrt` gives
    /// `Wort` before `Wirrt`. What is German stays here: names go last, guesses
    /// that are not close are dropped, and the capital is kept.
    fn get_suggestions(&self, word: &[char]) -> Vec<Vec<char>> {
        let hints = &*GERMAN_HINTS;
        let mut ranked: Vec<&[char]> =
            suggest_correct_spelling_with_hints(word, 100, 2, &self.dictionary, hints);

        // A name is the likelier guess only when nothing else is close: *Wrot* is *Wort*, not *Wroth*.
        // A stable sort keeps the core's order within each group.
        ranked.sort_by_key(|candidate| {
            self.dictionary
                .get_word_metadata(candidate)
                .is_some_and(|m| m.noun.as_ref().is_some_and(|n| n.is_proper == Some(true)))
        });

        // Writers rarely get the first letter wrong, and one edit is far likelier than two.
        // Once a candidate is one edit away and keeps the first letter, the guesses that need
        // two edits or a different first letter are noise: *Gaarten* is *Garten*, not
        // *Haarten*, *Paarten* or *Gameten*; *Hauss* is not *Ahauss*. An alternation from the
        // table counts as one edit wherever it is.
        let is_close = |candidate: &[char]| {
            hints.classify(word, candidate) != EditKind::Other
                && keeps_first_letter(word, candidate)
        };
        if ranked.iter().any(|candidate| is_close(candidate)) {
            ranked.retain(|candidate| {
                is_close(candidate)
                    || matches!(
                        hints.classify(word, candidate),
                        EditKind::Map | EditKind::Rep
                    )
            });
        }

        // *wichten* and *Wichten* are two words but one suggestion. Each keeps the
        // place of its best-ranked spelling, and takes the lower-case spelling
        // when the writer typed lower case.
        let lower_initial = |w: &[char]| w.first().is_some_and(|c| c.is_lowercase());
        let mut suggestions: Vec<Vec<char>> = Vec::new();
        for candidate in ranked {
            let folded: Vec<char> = candidate.iter().flat_map(|c| c.to_lowercase()).collect();
            let same = suggestions.iter_mut().find(|kept| {
                kept.iter()
                    .flat_map(|c| c.to_lowercase())
                    .eq(folded.iter().copied())
            });
            match same {
                Some(kept) => {
                    if lower_initial(word) && lower_initial(candidate) && !lower_initial(kept) {
                        *kept = candidate.to_vec();
                    }
                }
                None => suggestions.push(candidate.to_vec()),
            }
        }

        // Preserve the input's capitalization. German nouns and sentence-initial
        // words are capitalized, so a capitalized misspelling ("Worrt") implies a
        // capitalized correction ("Wort").
        if word.first().is_some_and(|c| c.is_uppercase()) {
            for suggestion in suggestions.iter_mut() {
                let has_internal_caps = suggestion.iter().skip(1).any(|c| c.is_uppercase());
                if !has_internal_caps && let Some(first) = suggestion.first_mut() {
                    *first = first.to_uppercase().next().unwrap();
                }
            }
        }
        // *Garten* and *garten* are two words, and after the capital is kept they are one
        // suggestion.
        let mut seen = HashSet::new();
        suggestions.retain(|suggestion| seen.insert(suggestion.clone()));

        // Also try a simple capitalization fix (common German error).
        if suggestions.is_empty() && word.len() > 1 {
            let mut capitalized = word.to_vec();
            if let Some(first_char) = capitalized.first_mut() {
                *first_char = first_char.to_uppercase().next().unwrap_or(*first_char);
            }
            if self.dictionary.contains_word(&capitalized) {
                suggestions.push(capitalized);
            }
        }

        suggestions.truncate(5);
        suggestions
    }
}

impl<T: Dictionary> Linter for GermanSpellCheck<T> {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();

        for paragraph in document.iter_paragraphs() {
            for sentence in paragraph.iter_sentences() {
                let tokens: Vec<&Token> = sentence
                    .iter()
                    .filter(|t| !t.kind.is_whitespace())
                    .collect();

                for (index, word) in tokens.iter().enumerate() {
                    if !matches!(word.kind, TokenKind::Word(_)) {
                        continue;
                    }
                    let word_chars = document.get_span_content(&word.span);

                    // Skip initialisms, Roman numerals and single letters.
                    if is_non_lexical_token(word_chars) {
                        continue;
                    }

                    // Skip words in dictionary
                    if self.dictionary.contains_word(word_chars) {
                        continue;
                    }

                    // An abbreviation German writes with a full stop: `Nr.`,
                    // `Dr.`, `Okt.`. The tokenizer hands over the letters and
                    // the stop separately, and the dictionary lists the
                    // abbreviation with its stop, so the bare letters stay a
                    // misspelling and never act as a compound element.
                    if tokens.get(index + 1).is_some_and(|next| {
                        matches!(next.kind, TokenKind::Punctuation(Punctuation::Period))
                            && next.span.start == word.span.end
                    }) && self.is_abbreviation_with_stop(word_chars)
                    {
                        continue;
                    }

                    // *geht's*, *gibt's*, *wie's*: the pronoun *es* cliticized
                    // onto a verb or a conjunction. The tokenizer keeps it on
                    // the word, so the host is what has to be a word.
                    if self.is_cliticized_es(word_chars) {
                        continue;
                    }

                    // *Planungs- und Bauphase*: the first half of a suspended compound is a begin form.
                    if document.get_full_content().get(word.span.end) == Some(&'-')
                        && crate::spell::compound_checker::is_listed_begin_form(word_chars)
                    {
                        continue;
                    }

                    // Try compound word splitting
                    if self.try_compound_word_check(word_chars) {
                        continue;
                    }

                    // A German dictionary has nothing to say about a sentence
                    // that has stopped being German. See
                    // [`german_foreign_stretch`] for the evidence it asks for.
                    if german_foreign_stretch::in_foreign_stretch(
                        &tokens,
                        index,
                        document,
                        |token| token.kind.is_determiner(),
                    ) {
                        continue;
                    }

                    // Get spelling suggestions
                    let suggestions = self.get_suggestions(word_chars);
                    let word_str: String = word_chars.iter().collect();

                    let message = if !suggestions.is_empty() {
                        let suggestions_str: Vec<String> = suggestions
                            .iter()
                            .map(|s| s.iter().collect::<String>())
                            .collect();
                        format!(
                            "»{}« könnte falsch geschrieben sein. Meinten Sie: {}?",
                            word_str,
                            suggestions_str.join(", ")
                        )
                    } else {
                        format!("»{}« ist kein bekanntes Wort.", word_str)
                    };

                    lints.push(Lint {
                        span: word.span,
                        lint_kind: LintKind::Spelling,
                        suggestions: suggestions
                            .into_iter()
                            .map(Suggestion::ReplaceWith)
                            .collect(),
                        priority: 20,
                        message,
                    });
                }
            }
        }

        lints
    }

    fn description(&self) -> &str {
        "Prüft die Rechtschreibung deutscher Wörter."
    }
}

#[cfg(test)]
mod tests {
    use super::{GermanSpellCheck, is_non_lexical_token};
    use crate::dialects::GermanDialect;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;
    use harper_core::spell::Dictionary;

    fn lint_text(text: &str) -> Vec<String> {
        use crate::linting::new_curated_german;
        use crate::spell::combined_german_dictionary;
        let dict = combined_german_dictionary();
        let mut linter = new_curated_german(GermanDialect::Standard, dict.clone());
        let document = Document::new(text, &PlainGerman, &dict);

        linter
            .lint(&document)
            .into_iter()
            .map(|lint| lint.message)
            .collect()
    }

    fn recognizes_compound(word: &str) -> bool {
        let dict = combined_german_dictionary();
        let spellcheck = GermanSpellCheck::new(dict);
        let chars: Vec<char> = word.chars().collect();

        spellcheck.try_compound_word_check(&chars)
    }

    #[test]
    fn recognizes_recursive_compounds() {
        for word in [
            "Gartenhaus",
            "Arbeitsstelle",
            "Frühstücksspeck",
            "Straßenrand",
            "Festplattenspeicher",
        ] {
            assert!(
                recognizes_compound(word),
                "{word} should be treated as a valid compound"
            );
        }
    }

    #[test]
    fn does_not_accept_misspelled_compounds() {
        for word in [
            "Festplattenspeicer",
            "Arbeitsplaz",
            "Straßenrant",
            // A capitalized compound does not end in a finite verb form.
            "Ausrieb",
            // The linking element is part of the compound: Arbeits-, Versions-.
            "Arbeitspeicher",
            "Versionhinweis",
            // A particle takes no linking element.
            "Absschnitte",
            // `et` is a word, but never a compound element.
            "Jahrhundet",
        ] {
            assert!(
                !recognizes_compound(word),
                "{word} should not be treated as a valid compound"
            );
        }
    }

    #[test]
    fn recognizes_simple_compounds() {
        for word in [
            "Gartenhaus",
            "Arbeitsstelle",
            "Straßenrand",
            "Zeiträume",
            "Arbeitsspeicher",
            "Haltestelle",
            "Lymphknoten",
            "Wohlbefinden",
        ] {
            assert!(
                recognizes_compound(word),
                "{word} should be treated as a valid compound"
            );
        }
    }

    #[test]
    fn lint_allows_festplattenspeicher() {
        let messages = lint_text("Der Festplattenspeicher ist fast voll.");

        assert!(
            messages
                .iter()
                .all(|message| !message.contains("Festplattenspeicher")),
            "Festplattenspeicher should not be flagged: {messages:?}"
        );
    }

    #[test]
    fn lint_flags_misspelled_storage_compounds() {
        let messages = lint_text("Der Festplattenspeicer ist fast voll.");

        assert!(
            messages
                .iter()
                .any(|message| message.contains("Festplattenspeicer")),
            "Misspelled compound should still be flagged: {messages:?}"
        );
    }

    #[test]
    fn lint_allows_common_technical_compounds() {
        let messages = lint_text(
            "Die Systemvoraussetzungen sind dokumentiert. \
             Das Betriebssystem nutzt eine Konfigurationsdatei im Texteditor zur Fehlerbehebung. \
             Ihre Unterschrift und die Unterschriften sind vorhanden.",
        );

        for word in [
            "Systemvoraussetzungen",
            "Betriebssystem",
            "Konfigurationsdatei",
            "Texteditor",
            "Fehlerbehebung",
            "Unterschrift",
            "Unterschriften",
        ] {
            assert!(
                messages.iter().all(|message| !message.contains(word)),
                "{word} should not be flagged: {messages:?}"
            );
        }
    }

    #[test]
    fn curated_german_uses_german_spellcheck_instead_of_generic_spellcheck() {
        use crate::linting::new_curated_german;
        use crate::spell::curated_german_dictionary;
        let linter = new_curated_german(GermanDialect::Standard, curated_german_dictionary());

        assert!(linter.config.is_rule_enabled("GermanSpellCheck"));
        assert!(!linter.config.is_rule_enabled("SpellCheck"));
    }

    fn suggestions_for(word: &str) -> Vec<String> {
        let dict = combined_german_dictionary();
        let spellcheck = GermanSpellCheck::new(dict);
        let chars: Vec<char> = word.chars().collect();
        spellcheck
            .get_suggestions(&chars)
            .into_iter()
            .map(|s| s.into_iter().collect())
            .collect()
    }

    /// One edit that keeps the first letter beats everything that does not.
    #[test]
    fn drops_far_fetched_suggestions() {
        assert_eq!(suggestions_for("Gaarten"), vec!["Garten"]);
        let hauss = suggestions_for("Hauss");
        assert_eq!(hauss.first().map(String::as_str), Some("Haus"));
        assert!(!hauss.iter().any(|s| s == "Ahauss"), "{hauss:?}");
        assert!(hauss.iter().all(|s| s.starts_with('H')), "{hauss:?}");
    }

    /// How often the intended word is the first suggestion, and among the first
    /// five, over a file of misspellings: one `kind<TAB>typo<TAB>intended` per line.
    /// Prints one line per kind and one for all.
    ///
    /// ```bash
    /// HARPER_TYPO_EVAL=typos.tsv cargo test --release --lib suggestion_accuracy -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "reads a file named by the environment"]
    fn suggestion_accuracy() {
        let Ok(path) = std::env::var("HARPER_TYPO_EVAL") else {
            return;
        };
        let spellcheck = GermanSpellCheck::new(crate::spell::compound_aware_german_dictionary());
        let mut stats: std::collections::BTreeMap<String, (usize, usize, usize)> =
            std::collections::BTreeMap::new();
        for line in std::fs::read_to_string(path).unwrap().lines() {
            let mut fields = line.split('\t');
            let (Some(kind), Some(typo), Some(intended)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let chars: Vec<char> = typo.chars().collect();
            if spellcheck.dictionary.contains_word(&chars) {
                continue;
            }
            let suggestions: Vec<String> = spellcheck
                .get_suggestions(&chars)
                .into_iter()
                .map(|s| s.into_iter().collect())
                .collect();
            let top1 = suggestions.first().is_some_and(|s| s == intended);
            let top5 = suggestions.iter().take(5).any(|s| s == intended);
            for key in [kind, "ALL"] {
                let entry = stats.entry(key.to_owned()).or_default();
                entry.0 += 1;
                entry.1 += usize::from(top1);
                entry.2 += usize::from(top5);
            }
        }
        for (kind, (n, top1, top5)) in stats {
            println!(
                "{kind:14} {n:5}  top1 {:5.1}%  top5 {:5.1}%",
                100.0 * top1 as f64 / n as f64,
                100.0 * top5 as f64 / n as f64
            );
        }
    }

    /// Slips of fast typing, from the hint table: a neighbouring key, a doubled
    /// letter typed once, and the same word in two capitalizations offered once.
    #[test]
    fn ranks_the_slips_of_fast_typing() {
        assert_eq!(
            suggestions_for("Nebem").first().map(String::as_str),
            Some("Neben")
        );
        assert_eq!(
            suggestions_for("muste").first().map(String::as_str),
            Some("musste")
        );
        assert_eq!(
            suggestions_for("Ergebnise").first().map(String::as_str),
            Some("Ergebnisse")
        );
        let wichtgen = suggestions_for("wichtgen");
        let folded: Vec<String> = wichtgen.iter().map(|s| s.to_lowercase()).collect();
        let mut unique = folded.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(folded.len(), unique.len(), "{wichtgen:?}");
        assert!(wichtgen.iter().any(|s| s == "wichtigen"), "{wichtgen:?}");
    }

    /// These expectations mirror hunspell's de_DE suggestion ordering: the
    /// intended word is a deletion, transposition, or orthographic alternation
    /// away from the typo, and those error kinds must outrank plain letter
    /// substitutions.
    #[test]
    fn suggests_intended_word_first() {
        for (typo, intended) in [
            ("Worrt", "Wort"),             // doubled letter (deletion)
            ("Hundd", "Hund"),             // doubled letter (deletion)
            ("Gartehn", "Garten"),         // stray letter (deletion)
            ("Sprechenn", "Sprechen"),     // doubled letter (deletion)
            ("flasch", "falsch"),          // transposition
            ("Wrot", "Wort"),              // transposition
            ("geschriben", "geschrieben"), // i -> ie alternation
        ] {
            let suggestions = suggestions_for(typo);
            assert_eq!(
                suggestions.first().map(String::as_str),
                Some(intended),
                "expected '{intended}' as the top suggestion for '{typo}', got {suggestions:?}"
            );
        }
    }

    #[test]
    fn suggested_words_are_valid_dictionary_words() {
        let dict = combined_german_dictionary();
        let spellcheck = GermanSpellCheck::new(dict.clone());

        for typo in [
            "Worrt",
            "flasch",
            "geschriben",
            "Gartehn",
            "Hundd",
            "Sprechenn",
        ] {
            let chars: Vec<char> = typo.chars().collect();
            let suggestions = spellcheck.get_suggestions(&chars);

            assert!(!suggestions.is_empty(), "expected suggestions for '{typo}'");
            for suggestion in &suggestions {
                assert!(
                    dict.contains_word(suggestion),
                    "suggestion '{}' for '{typo}' is not a valid German word",
                    suggestion.iter().collect::<String>()
                );
            }
        }
    }

    #[test]
    fn preserves_capitalization_of_capitalized_typo() {
        let suggestions = suggestions_for("Worrt");
        assert_eq!(
            suggestions.first().map(String::as_str),
            Some("Wort"),
            "got {suggestions:?}"
        );
    }

    fn spelling_reports(text: &str) -> Vec<String> {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanSpellCheck::new(dict)
            .lint(&document)
            .into_iter()
            .map(|lint| document.get_span_content_str(&lint.span))
            .collect()
    }

    #[test]
    fn an_abbreviation_needs_its_full_stop() {
        for text in [
            "Das Haus Nr. 13 steht leer.",
            "Ruf an unter Tel. 0611 123456.",
            "Berlin, den 29. Okt. 2011",
            "Dr. Meier und Prof. Schulz kommen.",
            "Er wohnt in der Bahnhofstr. 5.",
            "Siehe Abb. 3 und Kap. 2.",
            "Das gilt bes. für Kinder.",
        ] {
            assert!(
                spelling_reports(text).is_empty(),
                "{text}: {:?}",
                spelling_reports(text)
            );
        }
        // Without the stop the letters are no word, and they make no compound.
        assert_eq!(spelling_reports("Das Haus Nr 13 steht leer."), ["Nr"]);
        assert_eq!(spelling_reports("Ruf mich an, Tel 0611."), ["Tel"]);
        // The abbreviation of a noun is capitalized, like the noun.
        assert_eq!(spelling_reports("Lieber Herr dr. Müller"), ["dr"]);
    }

    #[test]
    fn a_cliticized_es_is_accepted_on_a_verb() {
        for text in [
            "Wie geht's dir?",
            "Gibt's das?",
            "Das war's.",
            "Sag's ihm!",
            "Wenn's regnet, bleiben wir.",
            "Das tue ich nicht.",
        ] {
            assert!(
                spelling_reports(text).is_empty(),
                "{text}: {:?}",
                spelling_reports(text)
            );
        }
        assert_eq!(spelling_reports("Das ist Peter's Auto."), ["Peter's"]);
        assert_eq!(spelling_reports("Er war stehts bemüht."), ["stehts"]);
        assert_eq!(
            spelling_reports("Du machts es mir nicht leicht."),
            ["machts"]
        );
    }

    #[test]
    fn word_internal_capital_marks_an_abbreviation() {
        for token in ["gGmbH", "UdSSR", "RoHS", "kV", "dB", "mA", "CaO"] {
            let chars: Vec<char> = token.chars().collect();
            assert!(
                is_non_lexical_token(&chars),
                "{token} should not be spell-checked"
            );
        }
    }

    #[test]
    fn ordinary_words_are_still_spell_checked() {
        // No word-internal capital, or too long for the abbreviation rule.
        for token in ["Wort", "wort", "Fußball", "Gebäudeversicherng"] {
            let chars: Vec<char> = token.chars().collect();
            assert!(
                !is_non_lexical_token(&chars),
                "{token} should be spell-checked"
            );
        }
    }
}
