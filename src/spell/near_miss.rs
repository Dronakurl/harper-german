//! The everyday typo that the compound splitter takes for a word.
//!
//! German compounds freely, so the spell checker accepts any word it can cut
//! into dictionary pieces, and a typo often cuts too: *zusamen* is *zu* +
//! *Samen*, *Beisiel* is *Bei* + *Siel*, *Funkion* is *Fun* + *Kion*. What
//! gives such a word away is a plain word one keystroke from it.
//!
//! Two kinds of keystroke are told apart:
//!
//! * the slips German spelling invites — a doubled consonant typed once
//!   (*Kontrole*), the silent *h* left out (*Ausname*), *ie* written *i*
//!   (*Mitglider*), one *s* of a linking *s* before an *s* (*Betriebsystem*),
//!   a letter typed twice (*assisstiere*, *diesselbe*) and a syllable typed
//!   twice (*Vorsitzendenden*).
//!   A word one of these slips away from a plain word is reported whatever
//!   its cut looks like;
//! * any letter missing or two letters swapped (*Literaur*, *Wahrehit*). That
//!   is a near miss for many real compounds too (*Tierart* and *Tierarzt*), so
//!   it counts only when the word does not cut cleanly into two plain words
//!   (*Fun* + *kion* is no clean cut, *Tier* + *Art* is).
//!
//! The near word must be a plain entry of the base dictionary, not itself a
//! compound guess, and the change may not touch the first or the last letter:
//! a different ending is another form of the same word (*Hingabe* and
//! *Hingaben*), not a typo of it. For the second kind, a word that is a verb
//! particle in front of a plain word (*abzielt*, *Hinblick*) is a word, not a
//! slip.

use harper_core::spell::Dictionary;

use super::curated_german_dictionary;

/// The letters a missing one is tried from.
const ALPHABET: &str = "abcdefghijklmnopqrstuvwxyzäöüß";

/// The pieces of a cut that are linking elements, not words.
const INTERFIXES: &[&str] = &["s", "es", "n", "en", "e", "er", "ns"];

/// The particles and prefixes that open a word in front of a plain word.
const PARTICLES: &[&str] = &[
    "ab", "an", "auf", "aus", "bei", "ein", "mit", "nach", "vor", "zu", "hin", "her", "rück",
    "wieder", "weg", "fort", "los", "zer", "be", "ge", "er", "ver", "ent", "dar", "um", "über",
    "unter", "durch",
];

/// The shortest near word worth offering: below it, too many short words lie
/// one letter apart.
const MIN_NEAR_WORD_LEN: usize = 6;

/// The slips German spelling invites, each as the word it repairs to.
fn spelling_slips(lower: &[char]) -> Vec<Vec<char>> {
    let vowel = |c: char| "aeiouäöü".contains(c);
    let inserted = |at: usize, letter: char| {
        let mut repaired = lower.to_vec();
        repaired.insert(at, letter);
        repaired
    };
    let mut slips = Vec::new();
    // *diesselbe*, *assisstiere*: a letter typed twice.
    // A consonant, and not the last letter: *Generaal* and *naturall* are
    // other languages.
    for at in 1..lower.len().saturating_sub(1) {
        if lower[at] == lower[at - 1] && "bcdfghklmnprstz".contains(lower[at]) {
            let mut repaired = lower.to_vec();
            repaired.remove(at);
            slips.push(repaired);
        }
    }
    // *Vorsitzendenden*: a syllable typed twice.
    for len in 3..=lower.len() / 2 {
        for at in 0..=lower.len() - 2 * len {
            // *anzuwendenden* is the gerundive of *anwenden*, not a slip.
            let gerundive = lower[..at].windows(2).any(|w| w == ['z', 'u'])
                && matches!(
                    lower[at..at + len],
                    ['d', 'e', 'n'] | ['e', 'n', 'd'] | ['n', 'd', 'e']
                );
            if !gerundive && lower[at..at + len] == lower[at + len..at + 2 * len] {
                let mut repaired = lower.to_vec();
                repaired.drain(at..at + len);
                slips.push(repaired);
            }
        }
    }
    for (at, &c) in lower.iter().enumerate().skip(1) {
        let before = lower[at - 1];
        // *Kontrole*, *Komision*: the consonant after a short vowel doubled.
        if "lmnfprtks".contains(c) && vowel(before) {
            slips.push(inserted(at, c));
        }
        // *Betriebsystem*: the linking *s* in front of an *s*. Not after
        // *-ung*, whose *Vergnügungsteuer* is a spelling of its own.
        if c == 's'
            && at > 2
            && !vowel(before)
            && before != 's'
            && !lower[..at].ends_with(&['u', 'n', 'g'])
        {
            slips.push(inserted(at, 's'));
        }
        // *Ausname*, *wärend*: the silent *h*.
        if "lmnr".contains(c) && vowel(before) && before != 'i' {
            slips.push(inserted(at, 'h'));
        }
        // *Mitglider*: *ie* written *i*.
        if c == 'i' && lower.get(at + 1).is_some_and(|&next| next != 'e') {
            slips.push(inserted(at + 1, 'e'));
        }
    }
    slips
}

/// Every word one missing letter or one swap away, leaving the first and the
/// last letter alone.
fn keystroke_slips(lower: &[char]) -> Vec<Vec<char>> {
    let mut slips = Vec::new();
    for at in 1..lower.len().saturating_sub(1) {
        if at + 2 < lower.len() {
            let mut swapped = lower.to_vec();
            swapped.swap(at, at + 1);
            slips.push(swapped);
        }
        for letter in ALPHABET.chars() {
            let mut repaired = lower.to_vec();
            repaired.insert(at, letter);
            slips.push(repaired);
        }
    }
    slips
}

/// The short words that stand as a compound element in their own right:
/// *Schrift|art*, *Nach|zug*, *Alt|bau*.
const SOLID_SHORT_ELEMENTS: &[&str] = &[
    "art", "bau", "zug", "typ", "tag", "ort", "amt", "weg", "akt", "rat", "ehe", "uhr", "see",
    "hof", "rad", "tor", "arm", "bus", "eis", "gas", "öl", "tat", "tal",
];

/// Does `lower` cut cleanly into two plain words, each of four letters or a
/// solid short element, the first maybe with a linking element? A compound
/// like that is a word, however close it lies to another (*Tierart*,
/// *Tierarzt*). A typo cuts only into scraps: *Fun* + *kion*, *Lit* + *eraur*.
fn has_a_clean_cut(base: &impl Dictionary, lower: &[char]) -> bool {
    let solid = |piece: &[char]| {
        piece.len() >= 4
            || SOLID_SHORT_ELEMENTS.contains(&piece.iter().collect::<String>().as_str())
    };
    let word = |piece: &[char]| solid(piece) && base.contains_word(piece);
    let stem = |piece: &[char]| {
        word(piece)
            || INTERFIXES.iter().any(|interfix| {
                let interfix: Vec<char> = interfix.chars().collect();
                piece.strip_suffix(interfix.as_slice()).is_some_and(word)
            })
    };
    (3..lower.len().saturating_sub(2)).any(|at| word(&lower[at..]) && stem(&lower[..at]))
}

/// The plain words `word` is a likely typo of, in the writer's case. Empty
/// when `word` reads as a word in its own right. Only meant for a word the
/// dictionary does not list and the compound splitter accepts.
pub fn near_misses(word: &[char]) -> Vec<Vec<char>> {
    if word.len() < MIN_NEAR_WORD_LEN - 1 || !word.iter().all(|c| c.is_alphabetic()) {
        return Vec::new();
    }
    let base = curated_german_dictionary();
    let lower: Vec<char> = word.iter().flat_map(|c| c.to_lowercase()).collect();
    // A name is no near word: names come in variants (*Hellmut*, *Helmut*).
    let is_plain_word = |candidate: &[char]| {
        candidate.len() >= MIN_NEAR_WORD_LEN
            && base.contains_word(candidate)
            && !base
                .get_word_metadata(candidate)
                .is_some_and(|m| m.noun.as_ref().is_some_and(|n| n.is_proper == Some(true)))
    };

    let mut found: Vec<Vec<char>> = spelling_slips(&lower)
        .into_iter()
        .filter(|candidate| is_plain_word(candidate))
        .collect();
    let behind_a_particle = || {
        PARTICLES.iter().any(|particle| {
            let particle: Vec<char> = particle.chars().collect();
            lower
                .strip_prefix(particle.as_slice())
                .is_some_and(|rest| rest.len() >= 4 && base.contains_word(rest))
        })
    };
    if found.is_empty() && !has_a_clean_cut(&*base, &lower) && !behind_a_particle() {
        // A spelled-out umlaut is no keystroke slip: *Werdens* is not
        // *Woerdens*.
        let new_digraph = |candidate: &[char]| {
            [['a', 'e'], ['o', 'e'], ['u', 'e']].iter().any(|digraph| {
                candidate.windows(2).any(|w| w == digraph)
                    && !lower.windows(2).any(|w| w == digraph)
            })
        };
        found = keystroke_slips(&lower)
            .into_iter()
            .filter(|candidate| is_plain_word(candidate) && !new_digraph(candidate))
            .collect();
    }
    found.sort();
    found.dedup();

    // The writer's case: a capital stays, and the rest is left to the
    // capitalization rules.
    if word[0].is_uppercase() {
        for candidate in &mut found {
            candidate[0] = candidate[0].to_uppercase().next().unwrap_or(candidate[0]);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::near_misses;

    fn near(word: &str) -> Vec<String> {
        let chars: Vec<char> = word.chars().collect();
        near_misses(&chars)
            .iter()
            .map(|candidate| candidate.iter().collect())
            .collect()
    }

    #[test]
    fn finds_the_word_behind_a_typo_that_cuts_like_a_compound() {
        for (typo, word) in [
            ("zusamen", "zusammen"),
            ("Kontrole", "Kontrolle"),
            ("Ausname", "Ausnahme"),
            ("Mitglider", "Mitglieder"),
            ("Betriebsystem", "Betriebssystem"),
            ("vieleicht", "vielleicht"),
            ("Wahrhit", "Wahrheit"),
            ("diesselbe", "dieselbe"),
            ("assisstiere", "assistiere"),
            ("Vorsitzendenden", "Vorsitzenden"),
            ("Inbesondere", "Insbesondere"),
            ("Literaur", "Literatur"),
            ("Funkion", "Funktion"),
        ] {
            assert!(
                near(typo).contains(&word.to_string()),
                "{typo}: {:?}",
                near(typo)
            );
        }
    }

    #[test]
    fn leaves_real_compounds_and_prefixed_words_alone() {
        for word in [
            "Holzwirtschaft",
            "Lebensstil",
            "Industrieländer",
            "Hinblick",
            "abzielt",
            "Hingabe",
            "Vorwegnahme",
            "Tierart",
            "Kombination",
            "Vergnügungsteuer",
            "Schriftarten",
            "primitiveren",
            "Werdens",
        ] {
            assert!(near(word).is_empty(), "{word}: {:?}", near(word));
        }
    }
}
