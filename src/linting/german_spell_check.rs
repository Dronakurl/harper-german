use std::sync::LazyLock;

use hashbrown::{HashMap, HashSet};

use crate::dialects::GermanDialect;
use crate::linting::german_foreign_stretch;
use crate::linting::german_relative_clause_comma::GermanRelativeClauseComma;
use crate::spell::compound_checker::{
    MIN_COMPOUND_PART_LEN, can_head_a_capitalized_compound, can_head_a_lowercase_compound,
    has_content_reading, has_inner_capital, interfix_fits, interfix_matches_known_begin,
    is_derivational_suffix, lowercase, may_be_compound_element, may_open_a_compound,
    may_open_the_word, may_take_an_interfix,
};
use crate::spell::curated_german_dictionary;
use crate::spell::german_dict::{
    GERMAN_BARRED_ELEMENTS, GERMAN_FUNCTION_WORDS, GERMAN_STEMS, GERMAN_SUFFIXED_ELEMENTS,
};
use crate::spell::near_miss::near_misses;
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

/// The articles, which end no compound: *aufdem* is *auf dem*.
const ARTICLES: &[&str] = &[
    "der", "die", "das", "den", "dem", "des", "ein", "eine", "einen", "einem", "einer", "eines",
];
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

    // Harper joins single letters with their stops into one token: *z. B.*,
    // *d. h.*, *e. V.*, *u.a.*, the initial in *Beate H.*, the caption *(l.)*.
    if word.len() >= 2
        && word
            .chunks(2)
            .all(|pair| matches!(pair, [letter, '.'] if letter.is_alphabetic()))
    {
        return true;
    }

    // And these with their stop and the space inside: *vs.*, *et al.*
    if matches!(word, ['v', 's', '.'] | ['e', 't', ' ', 'a', 'l', '.']) {
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
    /// Swiss German writes *ss* where the others write *ß* (*Strasse*, *heisst*).
    swiss: bool,
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
        Self::for_dialect(dictionary, GermanDialect::Standard)
    }

    pub fn for_dialect(dictionary: T, dialect: GermanDialect) -> Self {
        Self {
            dictionary,
            swiss: dialect == GermanDialect::Swiss,
        }
    }

    /// Is `word` a name written in lower case (*putin*, *jörg*)? The dictionary looks words up
    /// whatever their case, and a name is a word only with its capital: in lower case it is far
    /// more often a typo (*reul* for *reue*).
    fn is_lowercase_name(&self, word: &[char]) -> bool {
        // A noun and nothing else: *draus* is an adverb too.
        let only_a_noun = |word: &[char], proper: bool| {
            self.dictionary.get_word_metadata(word).is_some_and(|m| {
                m.noun
                    .as_ref()
                    .is_some_and(|n| !proper || n.is_proper == Some(true))
                    && m.verb.is_none()
                    && m.adjective.is_none()
                    && m.adverb.is_none()
                    && m.pronoun.is_none()
                    && m.determiner.is_none()
                    && m.conjunction.is_none()
                    && !m.preposition
            })
        };
        let is_name =
            |word: &[char]| !self.dictionary.contains_exact_word(word) && only_a_noun(word, true);
        // The genitive (*grosses* from *Grosse*) does not carry the name's reading.
        word.first().is_some_and(|c| c.is_lowercase())
            && (is_name(word)
                || (only_a_noun(word, false) && word.strip_suffix(&['s']).is_some_and(is_name)))
    }

    /// Is `word` the Swiss spelling of a word written with *ß* elsewhere?
    /// Every *ss* may stand for *ß* or for itself (*Schlossstrasse*).
    fn is_swiss_spelling(&self, word: &[char]) -> bool {
        let seams: Vec<usize> = (1..word.len())
            .filter(|&at| word[at - 1..=at] == ['s', 's'])
            .collect();
        if seams.is_empty() || seams.len() > 4 {
            return false;
        }
        (1..1u32 << seams.len()).any(|choice| {
            let mut spelled = Vec::with_capacity(word.len());
            let mut at = 0;
            while at < word.len() {
                if let Some(bit) = seams.iter().position(|&seam| seam == at + 1)
                    && choice & (1 << bit) != 0
                {
                    spelled.push('ß');
                    at += 2;
                } else {
                    spelled.push(word[at]);
                    at += 1;
                }
            }
            self.dictionary.contains_word(&spelled) || self.try_compound_word_check(&spelled)
        })
    }

    /// The words of `tokens` that are part of a dictionary entry written with
    /// a hyphen: *Know-how*, *Play-offs*, *baden-württembergischen*,
    /// *Dipl.-Ing.* Harper splits these at the hyphen, and *how* or
    /// *württembergischen* alone are no German words. The entry has to match
    /// in its own case, but for the first letter: *Play-Offs* is a misspelling.
    fn in_hyphenated_entries(&self, tokens: &[&Token], document: &Document) -> HashSet<usize> {
        let mut known = HashSet::new();
        let touch = |at: usize| {
            tokens
                .get(at + 1)
                .is_some_and(|next| tokens[at].span.end == next.span.start)
        };
        let is_word = |at: usize| {
            tokens
                .get(at)
                .is_some_and(|t| matches!(t.kind, TokenKind::Word(_)))
        };
        let is_period = |at: usize| {
            tokens
                .get(at)
                .is_some_and(|t| matches!(t.kind, TokenKind::Punctuation(Punctuation::Period)))
        };
        let is_hyphen = |at: usize| tokens.get(at).is_some_and(|t| t.kind.is_hyphen());
        let is_entry = |text: &[char]| {
            let mut flipped = text.to_vec();
            if let Some(first) = flipped.first_mut() {
                *first = if first.is_uppercase() {
                    first.to_lowercase().next().unwrap_or(*first)
                } else {
                    first.to_uppercase().next().unwrap_or(*first)
                };
            }
            self.dictionary.contains_exact_word(text)
                || self.dictionary.contains_exact_word(&flipped)
        };

        let mut start = 0;
        while start < tokens.len() {
            // The words of a chain `word-word-…` (or `abbr.-word`) with nothing between them.
            let mut words = vec![start];
            while let Some(&last) = words.last()
                && is_word(last)
            {
                let next = if is_hyphen(last + 1) && touch(last) && touch(last + 1) {
                    last + 2
                } else if is_period(last + 1)
                    && is_hyphen(last + 2)
                    && touch(last)
                    && touch(last + 1)
                    && touch(last + 2)
                {
                    last + 3
                } else {
                    break;
                };
                if !is_word(next) {
                    break;
                }
                words.push(next);
            }
            for (i, &first) in words.iter().enumerate() {
                for &last in &words[i + 1..] {
                    let mut span =
                        harper_core::Span::new(tokens[first].span.start, tokens[last].span.end);
                    let mut found = is_entry(document.get_span_content(&span));
                    if !found && is_period(last + 1) && touch(last) {
                        span.end += 1;
                        found = is_entry(document.get_span_content(&span));
                    }
                    if found {
                        known.extend((first..=last).map(|at| tokens[at].span.start));
                    }
                }
            }
            start = words.last().map_or(start, |&last| last) + 1;
        }
        known
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
        // A function word takes no linking element; see `function_word_set`.
        if !interfix.is_empty()
            && GERMAN_FUNCTION_WORDS.contains(element)
            && !has_content_reading(&self.dictionary, element)
        {
            return None;
        }
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
                || (depth == 0 && !may_open_the_word(first_part, lowercase_whole.is_some()))
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
        if word.len() < MIN_COMPOUND_PART_LEN * 2 || has_inner_capital(word) {
            return false;
        }

        let mut memo = HashMap::new();
        self.is_valid_compound_segment(word, 0, &mut memo, lowercase(word).then_some(word))
    }

    /// `word` as two words with a space between, if both halves are words as
    /// written, and whether the seam is a sure one: a capital right behind it
    /// (*imKönigreich*) or a function word or finite verb in front of it
    /// (*sindschmal*) or an article behind it (*aufdem*), none of which a compound
    /// has.
    ///
    /// The first half needs two letters and the second three, and the split
    /// with the longer first half wins, so *dieselbe* is not offered as *die
    /// selbe*. Only reached for a word the dictionary has already rejected.
    fn two_words(&self, word: &[char]) -> Option<(Vec<char>, bool)> {
        if word.len() < 5 {
            return None;
        }
        // The base dictionary, so that neither half is itself a guess of the
        // compound splitter: *understanding* is not *und* + *erstanding*.
        let base = curated_german_dictionary();
        (2..=word.len() - 3).rev().find_map(|at| {
            let (left, right) = word.split_at(at);
            if !base.contains_exact_word(left) || !base.contains_exact_word(right) {
                return None;
            }
            let capital_seam = left.last().is_some_and(|c| c.is_lowercase())
                && right.first().is_some_and(|c| c.is_uppercase());
            // *sindschmal*: a conjunction or a finite *sein* opens no compound.
            // A preposition does, as the particle of a verb (*umcodiert*), so it
            // does not count here.
            let lower_left: Vec<char> = left.iter().flat_map(|c| c.to_lowercase()).collect();
            let function_left = !may_open_the_word(&lower_left, true);
            // *aufdem*: an article cannot end a compound either.
            let right_text: String = right.iter().collect();
            let article_right = ARTICLES.contains(&right_text.as_str());
            let sure = capital_seam || function_left || article_right;
            // Without a sure seam, only a capitalized second half is worth offering:
            // two lower-case words glued together is how most real compounds look.
            if !sure && !right.first().is_some_and(|c| c.is_uppercase()) {
                return None;
            }
            let joined: Vec<char> = left.iter().chain(&[' ']).chain(right).copied().collect();
            Some((joined, sure))
        })
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

        // Two words that lost the space between them: *sindschmal*, *imKönigreich*.
        // Ahead of the edits when the seam shows it — a capital in the middle, or a
        // function word in front, which no compound begins with — and behind the
        // best edit otherwise, so *Kunstoff* still offers *Kunststoff* first.
        if let Some((split, sure)) = self.two_words(word) {
            suggestions.retain(|suggestion| *suggestion != split);
            let at = if sure { 0 } else { suggestions.len().min(1) };
            suggestions.insert(at, split);
        }

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

        // *gesingt*, *geschreibt*: the weak participle of a strong verb. The
        // strong one changes the vowel of the stem and ends in *-en*.
        if let Some(strong) = self.strong_participle(word) {
            suggestions.retain(|suggestion| *suggestion != strong);
            suggestions.insert(0, strong);
        }

        suggestions.truncate(5);
        suggestions
    }

    /// The strong participle for a weak one built on a strong verb:
    /// *gesingt* → *gesungen*, *geschreibt* → *geschrieben*, *gelauft* →
    /// *gelaufen*. The stem's vowel is swapped for each vowel a strong verb
    /// can take, and only a participle the dictionary lists is offered.
    fn strong_participle(&self, word: &[char]) -> Option<Vec<char>> {
        const VOWELS: &[&str] = &["a", "e", "i", "o", "u", "ie", "ei", "au", "ä", "ö", "ü"];
        let text: String = word.iter().collect();
        let lower = text.to_lowercase();
        let stem = lower.strip_prefix("ge")?.strip_suffix('t')?;
        if stem.len() < 2 || !self.dictionary.contains_word_str(&format!("{stem}en")) {
            return None;
        }
        let is_vowel = |c: char| "aeiouäöü".contains(c);
        let start = stem.find(is_vowel)?;
        let end = stem[start..]
            .find(|c: char| !is_vowel(c))
            .map_or(stem.len(), |i| start + i);
        let (onset, coda) = (&stem[..start], &stem[end..]);
        let found: Vec<String> = VOWELS
            .iter()
            .map(|vowel| format!("ge{onset}{vowel}{coda}en"))
            .filter(|candidate| {
                self.dictionary
                    .get_word_metadata_str(candidate)
                    .is_some_and(|m| m.is_verb() || m.is_adjective())
            })
            .collect();
        // One strong participle, or the guess is not safe.
        let [participle] = found.as_slice() else {
            return None;
        };
        Some(participle.chars().collect())
    }
}

/// Is the capitalized word at `index` part of a name: behind a first name or
/// an initial (*Wolfgang Wieland*, *C. Pepper*), or in front of a hyphen
/// (*Hartree-Fock*)? Behind any capitalized content word, in fact: a noun
/// in front of the typo (*die Firma Kontrole*) is lost with it, which is rare. A name the dictionary does not know cuts like a compound
/// as readily as a typo does.
fn in_a_name(tokens: &[&Token], index: usize, document: &Document) -> bool {
    let word = tokens[index];
    if !document
        .get_span_content(&word.span)
        .first()
        .is_some_and(|c| c.is_uppercase())
    {
        return false;
    }
    if document.get_full_content().get(word.span.end) == Some(&'-') {
        return true;
    }
    let Some(previous) = index.checked_sub(1).map(|at| tokens[at]) else {
        return false;
    };
    let previous_text = document.get_span_content(&previous.span);
    let capital = previous_text.first().is_some_and(|c| c.is_uppercase());
    // *C. Pepper*, *Raymond L. Lindeman*: an initial, with its full stop
    // a token of its own or part of the word.
    let is_initial = |text: &[char]| matches!(text, [c] | [c, '.'] if c.is_uppercase());
    if is_initial(previous_text) {
        return true;
    }
    if matches!(previous.kind, TokenKind::Punctuation(Punctuation::Period))
        && index >= 2
        && is_initial(document.get_span_content(&tokens[index - 2].span))
    {
        return true;
    }
    // *Wolfgang Wieland*, *Volker Halbach*: a capitalized word in front that
    // is no article, preposition, pronoun or conjunction, and not the first
    // word of the sentence, whose capital says nothing.
    previous.kind.is_proper_noun()
        || (capital
            && index >= 2
            && !previous.kind.is_determiner()
            && !previous.kind.is_preposition()
            && !previous.kind.is_pronoun()
            && !previous.kind.is_conjunction())
}

/// The lint for a word the compound splitter accepts that is a slip away from a
/// plain word; see [`near_misses`].
fn near_miss_lint(word: &Token, chars: &[char]) -> Option<Lint> {
    let near = near_misses(chars);
    if near.is_empty() {
        return None;
    }
    let word_str: String = chars.iter().collect();
    let near_str: Vec<String> = near.iter().map(|s| s.iter().collect()).collect();
    Some(Lint {
        span: word.span,
        lint_kind: LintKind::Spelling,
        suggestions: near.into_iter().map(Suggestion::ReplaceWith).collect(),
        priority: 20,
        message: format!(
            "»{word_str}« ist vermutlich ein Tippfehler. Meinten Sie: {}?",
            near_str.join(", ")
        ),
    })
}

impl<T: Dictionary> Linter for GermanSpellCheck<T> {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();

        for paragraph in document.iter_paragraphs() {
            // Across sentences: Harper ends one at the stop of *Dr.-Ing.*
            let words: Vec<&Token> = paragraph
                .iter()
                .filter(|t| !t.kind.is_whitespace())
                .collect();
            let hyphenated = self.in_hyphenated_entries(&words, document);
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

                    if hyphenated.contains(&word.span.start) {
                        continue;
                    }

                    // Skip words in dictionary. One the base dictionary does
                    // not list got in as a compound, and a typo often cuts
                    // too (*zusamen* is *zu* + *Samen*): it is still reported
                    // when it is a slip away from a plain word.
                    if self.dictionary.contains_word(word_chars)
                        && !self.is_lowercase_name(word_chars)
                    {
                        if !curated_german_dictionary().contains_word(word_chars)
                            && !in_a_name(&tokens, index, document)
                        {
                            lints.extend(near_miss_lint(word, word_chars));
                        }
                        continue;
                    }

                    // An abbreviation German writes with a full stop: `Nr.`,
                    // `Dr.`, `Okt.`. The tokenizer hands over the letters and
                    // the stop separately, and the dictionary lists the
                    // abbreviation with its stop, so the bare letters stay a
                    // misspelling and never act as a compound element.
                    // The source is read, not the next token: in *usw..* the stops
                    // may come as one token.
                    let stop_follows = document.get_full_content().get(word.span.end) == Some(&'.');
                    if self.is_abbreviation_with_stop(word_chars) {
                        // Only the everyday ones, and only in German text: *mind*
                        // in an English title and *La* in *La Malfa* are words of
                        // their own.
                        const EVERYDAY: &[&str] = &[
                            "ca", "bzw", "usw", "evtl", "ggf", "vgl", "inkl", "zzgl", "bzgl",
                            "sog", "Nr", "Str", "Tel", "Mio", "Mrd", "Jh", "Abs", "Std", "Min",
                            "Hr", "Prof", "Dr",
                        ];
                        let written_word: String = word_chars.iter().collect();
                        if !stop_follows
                            && EVERYDAY.contains(&written_word.as_str())
                            && !german_foreign_stretch::in_foreign_stretch(
                                &tokens,
                                index,
                                document,
                                |token| token.kind.is_determiner(),
                            )
                        {
                            // *ca 20 Euro*: the abbreviation without its stop.
                            let written: String = word_chars.iter().collect();
                            lints.push(Lint {
                                span: word.span,
                                lint_kind: LintKind::Punctuation,
                                suggestions: vec![Suggestion::InsertAfter(vec!['.'])],
                                priority: 20,
                                message: format!(
                                    "Die Abkürzung »{written}.« schreibt man mit Punkt."
                                ),
                            });
                            continue;
                        }
                        if stop_follows {
                            continue;
                        }
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

                    // Try compound word splitting.
                    if self.try_compound_word_check(word_chars) {
                        if !in_a_name(&tokens, index, document) {
                            lints.extend(near_miss_lint(word, word_chars));
                        }
                        continue;
                    }

                    // *3-tägigen*, *20-prozentige*: the number stands for a numeral the word
                    // would otherwise be compounded with (*dreitägigen*).
                    if index >= 2
                        && tokens[index - 1].kind.is_hyphen()
                        && tokens[index - 1].span.end == word.span.start
                        && tokens[index - 2].kind.is_number()
                        && tokens[index - 2].span.end == tokens[index - 1].span.start
                        && word_chars.first().is_some_and(|c| c.is_lowercase())
                    {
                        let spelled: Vec<char> =
                            "drei".chars().chain(word_chars.iter().copied()).collect();
                        if self.dictionary.contains_word(&spelled)
                            || self.try_compound_word_check(&spelled)
                        {
                            continue;
                        }
                    }

                    if self.swiss && self.is_swiss_spelling(word_chars) {
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

                    // *am 3ten Mai*: an ordinal written with an ending. The
                    // ending alone is no word to correct; the number takes a
                    // full stop instead.
                    let lower: String = word_chars.iter().flat_map(|c| c.to_lowercase()).collect();
                    if let Some(number) = index.checked_sub(1).map(|i| tokens[i])
                        && matches!(number.kind, TokenKind::Number(_))
                        && number.span.end == word.span.start
                        && [
                            "te", "ten", "ter", "tes", "tem", "ste", "sten", "ster", "stes", "stem",
                        ]
                        .contains(&lower.as_str())
                    {
                        let digits = document.get_span_content_str(&number.span);
                        lints.push(Lint {
                            span: harper_core::Span::new(number.span.start, word.span.end),
                            lint_kind: LintKind::Formatting,
                            suggestions: vec![Suggestion::ReplaceWith(
                                format!("{digits}.").chars().collect(),
                            )],
                            priority: 20,
                            message: format!(
                                "Eine Ordnungszahl schreibt man mit Punkt: »{digits}.«, nicht »{digits}{lower}«."
                            ),
                        });
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
    fn an_abbreviation_without_its_stop_gets_one() {
        let dict = crate::spell::combined_german_dictionary();
        for (text, expected) in [
            ("Das kostet ca 20 Euro.", 1),
            ("Das kostet ca. 20 Euro.", 0),
            ("Wir brauchen Stifte, Papier usw..", 0),
        ] {
            let document = Document::new(text, &PlainGerman, &dict);
            let lints = GermanSpellCheck::new(dict.clone()).lint(&document);
            assert_eq!(lints.len(), expected, "{text}: {lints:?}");
        }
    }

    #[test]
    fn a_weak_participle_of_a_strong_verb_gets_the_strong_one() {
        assert_eq!(
            suggestions_for("gesingt").first().map(String::as_str),
            Some("gesungen")
        );
        assert_eq!(
            suggestions_for("geschreibt").first().map(String::as_str),
            Some("geschrieben")
        );
    }

    #[test]
    fn an_ordinal_with_an_ending_takes_a_full_stop() {
        let dict = crate::spell::combined_german_dictionary();
        let document = Document::new("Am 3ten Mai haben wir frei.", &PlainGerman, &dict);
        let lints = GermanSpellCheck::new(dict.clone()).lint(&document);
        assert_eq!(lints.len(), 1, "{lints:?}");
        assert_eq!(document.get_span_content_str(&lints[0].span), "3ten");
    }

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

    /// The intended word is a deletion, transposition, or orthographic alternation
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
    fn letters_with_their_stops_are_abbreviations() {
        for text in [
            "Das gilt z. B. für Kinder, d. h. für alle.",
            "Der Verein Musik e. V. lädt ein.",
            "Das war um 500 v. Chr. und u.a. in Rom.",
            "Zar Alexander I. regierte, sagt Beate H. (l.) im Bild.",
            "Müller et al. zeigen, dass A vs. B gilt.",
        ] {
            assert!(
                spelling_reports(text).is_empty(),
                "{text}: {:?}",
                spelling_reports(text)
            );
        }
    }

    #[test]
    fn an_entry_with_a_hyphen_is_one_word() {
        for text in [
            "Das Know-how fehlt.",
            "Die baden-württembergischen Banken sind stark.",
            "Sie gewannen die Play-offs mit viel Know-how-Transfer.",
        ] {
            assert!(
                spelling_reports(text).is_empty(),
                "{text}: {:?}",
                spelling_reports(text)
            );
        }
        assert!(spelling_reports("Nach der 3-tägigen Reise kam die 4-köpfige Familie.").is_empty());
        assert_eq!(spelling_reports("Nach der 3-tägign Reise."), ["tägign"]);
        let text = "Prof. Dr.-Ing. Meier und Dipl.-Ing. Schulz kommen.";
        assert!(
            spelling_reports(text).is_empty(),
            "{:?}",
            spelling_reports(text)
        );
        assert_eq!(spelling_reports("Ich bin ein Dipl. Ing."), ["Dipl", "Ing"]);
        // In the case of the entry: *Play-off*, not *Play-Off*.
        assert_eq!(
            spelling_reports("Die Play-Offs beginnen."),
            ["Play", "Offs"]
        );
        // Each part still has to be a word when the whole is no entry.
        assert_eq!(spelling_reports("Das Know-hwo fehlt."), ["Know", "hwo"]);
    }

    #[test]
    fn swiss_german_writes_ss_for_eszett() {
        let swiss = |text: &str| {
            let dict = combined_german_dictionary();
            let document = Document::new(text, &PlainGerman, &dict);
            GermanSpellCheck::for_dialect(dict, GermanDialect::Swiss)
                .lint(&document)
                .into_iter()
                .map(|lint| document.get_span_content_str(&lint.span))
                .collect::<Vec<_>>()
        };
        let text = "Es heisst, die grosse Strasse sei gemäss Plan geschlossen.";
        assert!(swiss(text).is_empty(), "{:?}", swiss(text));
        assert_eq!(
            spelling_reports(text),
            ["heisst", "grosse", "Strasse", "gemäss"]
        );
        assert_eq!(
            swiss("Die Strasse ist grossartig, die Strase nicht."),
            ["Strase"]
        );
    }

    #[test]
    fn a_name_is_a_word_only_with_its_capital() {
        assert!(spelling_reports("Gestern sprach Putin mit Jörg in Flensburg.").is_empty());
        assert_eq!(
            spelling_reports("Gestern sprach putin mit jörg."),
            ["putin", "jörg"]
        );
        assert_eq!(spelling_reports("Das war ein grosses Glück."), ["grosses"]);
        assert!(spelling_reports("Wir machen das Beste draus.").is_empty());
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
