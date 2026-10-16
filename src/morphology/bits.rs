//! Which bit of [`LanguageBits`] means what.
//!
//! Harper stores the bits and merges two entries with a bitwise OR, so every
//! reading a form can have gets a bit of its own: the OR of two forms is then
//! the union of their readings, which is what [`Morphology::or`] computes.
//!
//! The first word holds four 12-bit [`Agreement`]s, one per part of speech,
//! then the mood and the foreign-term flag:
//!
//! | bits    | content                                                         |
//! |---------|-----------------------------------------------------------------|
//! | 0–11    | noun: case 0–3, gender 4–6, number 7–8, person 9–11             |
//! | 12–23   | pronoun, same layout                                            |
//! | 24–35   | determiner, same layout                                         |
//! | 36–47   | verb, same layout                                               |
//! | 48–51   | mood: indicative, imperative, subjunctive I, subjunctive II     |
//! | 52      | foreign term                                                    |
//!
//! The second word is unused.
//!
//! An agreement with no feature set and an absent one are the same here. Every
//! accessor treats them alike ([`MorphologyExt`](super::MorphologyExt) falls back to
//! the empty agreement), so nothing reads the difference.

use harper_core::LanguageBits;

use super::{Agreement, CaseSet, GenderSet, Mood, Morphology, NumberSet, PersonSet};

const AGREEMENT_BITS: u32 = 12;
const NOUN: u32 = 0;
const PRONOUN: u32 = 12;
const DETERMINER: u32 = 24;
const VERB: u32 = 36;
const MOOD: u32 = 48;
const FOREIGN: u32 = 52;

const MOODS: [Mood; 4] = [
    Mood::Indicative,
    Mood::Imperative,
    Mood::SubjunctiveI,
    Mood::SubjunctiveII,
];

fn encode_agreement(agreement: &Agreement) -> u64 {
    agreement.case.bits() as u64
        | (agreement.gender.bits() as u64) << 4
        | (agreement.number.bits() as u64) << 7
        | (agreement.person.bits() as u64) << 9
}

fn decode_agreement(bits: u64) -> Option<Agreement> {
    let bits = bits & ((1 << AGREEMENT_BITS) - 1);
    (bits != 0).then(|| Agreement {
        case: CaseSet::from_bits_truncate((bits & 0xf) as u8),
        gender: GenderSet::from_bits_truncate((bits >> 4 & 0x7) as u8),
        number: NumberSet::from_bits_truncate((bits >> 7 & 0x3) as u8),
        person: PersonSet::from_bits_truncate((bits >> 9 & 0x7) as u8),
    })
}

/// The bits for `morphology`.
pub fn encode(morphology: &Morphology) -> LanguageBits {
    let agreement = |a: Option<Agreement>, at: u32| a.map_or(0, |a| encode_agreement(&a) << at);

    let mut word = agreement(morphology.noun, NOUN)
        | agreement(morphology.pronoun, PRONOUN)
        | agreement(morphology.determiner, DETERMINER)
        | agreement(morphology.verb, VERB);
    if let Some(mood) = morphology.mood {
        let index = MOODS.iter().position(|m| *m == mood).unwrap() as u32;
        word |= 1 << (MOOD + index);
    }
    if morphology.is_foreign == Some(true) {
        word |= 1 << FOREIGN;
    }

    LanguageBits([word, 0])
}

/// The morphology the bits describe, or `None` when no bit is set.
///
/// Should two moods have been merged into one entry, the first in [`MOODS`] wins.
pub fn decode(bits: LanguageBits) -> Option<Morphology> {
    if bits.is_empty() {
        return None;
    }
    let word = bits.0[0];

    Some(Morphology {
        noun: decode_agreement(word >> NOUN),
        pronoun: decode_agreement(word >> PRONOUN),
        determiner: decode_agreement(word >> DETERMINER),
        verb: decode_agreement(word >> VERB),
        mood: (0..MOODS.len() as u32)
            .find(|i| word & 1 << (MOOD + i) != 0)
            .map(|i| MOODS[i as usize]),
        is_foreign: (word & 1 << FOREIGN != 0).then_some(true),
    })
}

#[cfg(test)]
mod tests {
    use super::super::{Case, Gender, Number, Person};
    use super::*;

    fn agreements() -> impl Iterator<Item = Agreement> {
        (0..1u16 << AGREEMENT_BITS).filter_map(|bits| decode_agreement(bits as u64))
    }

    /// Every agreement the four axes allow survives the round trip, in every slot.
    #[test]
    fn every_agreement_round_trips() {
        let mut count = 0;
        for agreement in agreements() {
            count += 1;
            for slot in 0..4 {
                let mut morphology = Morphology::default();
                *[
                    &mut morphology.noun,
                    &mut morphology.pronoun,
                    &mut morphology.determiner,
                    &mut morphology.verb,
                ][slot] = Some(agreement);
                assert_eq!(decode(encode(&morphology)), Some(morphology));
            }
        }
        assert_eq!(count, (1 << AGREEMENT_BITS) - 1);
    }

    #[test]
    fn moods_and_the_foreign_flag_round_trip() {
        for mood in MOODS {
            let morphology = Morphology {
                mood: Some(mood),
                is_foreign: Some(true),
                ..Default::default()
            };
            assert_eq!(decode(encode(&morphology)), Some(morphology));
        }
    }

    #[test]
    fn nothing_is_no_bits() {
        assert!(encode(&Morphology::default()).is_empty());
        assert_eq!(decode(LanguageBits::default()), None);
    }

    /// Harper merges with an OR of the bits; that has to be what
    /// [`Morphology::or`] gives.
    #[test]
    fn or_of_the_bits_is_or_of_the_morphology() {
        let dat_sg = Morphology {
            noun: Some(Agreement {
                case: Case::Dative.into(),
                number: Number::Singular.into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let nom_pl_fem = Morphology {
            noun: Some(Agreement {
                case: Case::Nominative.into(),
                gender: Gender::Feminine.into(),
                number: Number::Plural.into(),
                ..Default::default()
            }),
            verb: Some(Agreement {
                person: Person::Third.into(),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_eq!(
            decode(encode(&dat_sg).or(&encode(&nom_pl_fem))),
            Some(dat_sg.or(&nom_pl_fem))
        );
    }

    #[test]
    fn the_axes_do_not_overlap() {
        let all = Agreement {
            case: CaseSet::all(),
            gender: GenderSet::all(),
            number: NumberSet::all(),
            person: PersonSet::all(),
        };
        assert_eq!(encode_agreement(&all), (1 << AGREEMENT_BITS) - 1);
    }
}
