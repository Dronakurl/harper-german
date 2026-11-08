use crate::lexing::lex_german_token;
use harper_core::Token;
use harper_core::lexing::lex_with;
use harper_core::parsers::{Parser, condense_abbreviations, condense_ordinals};

/// Abbreviations whose full stop does not end a sentence and whose letters are written with
/// spaces ("z. B.") or without ("z.B."). [`PlainGerman`] joins each into one token.
const ABBREVIATIONS: &[&str] = &[
    "z. B.", "d. h.", "d. i.", "u. a.", "u. Ä.", "u. U.", "o. Ä.", "i. d. R.", "i. A.", "i. V.",
    "i. e. S.", "m. E.", "s. o.", "s. u.", "z. T.", "z. Zt.", "e. V.", "a. D.", "n. Chr.",
    "v. Chr.", "p. a.", "bzw.", "ggf.", "usw.", "usf.", "vgl.", "evtl.", "insb.", "bspw.", "sog.",
    "etc.", "ebd.", "dgl.", "inkl.", "exkl.", "zzgl.", "abzgl.", "bzgl.", "zit.", "et al.", "vs.",
    "ca.", "Nr.", "Dr.", "Prof.", "Str.", "Abb.", "Tab.", "Kap.", "Bd.", "Hrsg.", "geb.", "gest.",
    "Mio.", "Mrd.",
];

/// The months, after which "8." is an ordinal ("am 8. November").
const MONTHS: &[&str] = &[
    "Januar",
    "Jänner",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
];

/// The abbreviations, plus each one that is written with spaces also without them.
fn abbreviations() -> Vec<String> {
    let mut all: Vec<String> = ABBREVIATIONS.iter().map(|a| (*a).to_owned()).collect();
    all.extend(
        ABBREVIATIONS
            .iter()
            .filter(|a| a.contains(". ") && !a.starts_with("et "))
            .map(|a| a.replace(' ', "")),
    );
    all
}

/// Whether "8." before `next` is an ordinal: a number ("8. 11."), a word in lower case
/// ("5. mal") or a month.
fn follows_ordinal(next: &[char]) -> bool {
    let Some(first) = next.first() else {
        return false;
    };
    first.is_ascii_digit()
        || first.is_lowercase()
        || MONTHS.iter().any(|m| m.chars().eq(next.iter().copied()))
}

/// A parser that will attempt to lex as many tokens as possible,
/// without discrimination and until the end of input.
///
/// Uses German-specific lexing that currently reuses the English lexing
/// logic but is structured to allow future German-specific tokenization
/// if needed. Then joins what German writes with a full stop that does not end the
/// sentence: abbreviations ("z. B.", "bzw.") and ordinals ("am 8. November").
#[derive(Clone, Copy)]
pub struct PlainGerman;

impl Parser for PlainGerman {
    fn parse(&self, source: &[char]) -> Vec<Token> {
        let mut tokens = lex_with(source, lex_german_token);
        let abbreviations = abbreviations();
        let abbreviations: Vec<&str> = abbreviations.iter().map(String::as_str).collect();
        condense_abbreviations(&mut tokens, source, &abbreviations);
        condense_ordinals(&mut tokens, source, follows_ordinal);
        tokens
    }

    /// Not English, so [`Document`](harper_core::Document) skips the Brill
    /// tagger and the neural chunker, which are English-only models.
    fn is_english(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use harper_core::parsers::Parser;
    use harper_core::{Document, TokenStringExt};

    use super::PlainGerman;

    fn sentences(text: &str) -> usize {
        let chars: Vec<char> = text.chars().collect();
        let doc = Document::new_from_chars(
            chars.into(),
            &PlainGerman,
            &harper_core::spell::FstDictionary::curated(),
        );
        doc.iter_sentences().count()
    }

    #[test]
    fn abbreviations_and_ordinals_do_not_end_the_sentence() {
        assert_eq!(sentences("Obst bzw. Gemüse ist gesund."), 1);
        assert_eq!(sentences("Das ist z. B. so."), 1);
        assert_eq!(sentences("Das ist z.B. so."), 1);
        assert_eq!(sentences("Wir treffen uns am 8. November."), 1);
        assert_eq!(sentences("Am 8. 11. um acht."), 1);
        // A number at the end of a sentence still ends it.
        assert_eq!(sentences("Ich bin 8. Er ist 9."), 2);
    }

    #[test]
    fn quotation_marks_are_paired() {
        let chars: Vec<char> = "Er sagte: „Ich komme morgen.“".chars().collect();
        let doc = Document::new_from_chars(
            chars.into(),
            &PlainGerman,
            &harper_core::spell::FstDictionary::curated(),
        );
        assert!(
            doc.iter_quotes()
                .all(|q| q.kind.as_quote().unwrap().twin_loc.is_some())
        );
        assert_eq!(doc.iter_quotes().count(), 2);
        let _ = PlainGerman.is_english();
    }
}
