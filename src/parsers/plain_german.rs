use crate::lexing::lex_german_token;
use harper_core::Token;
use harper_core::lexing::lex_with;
use harper_core::parsers::Parser;

/// A parser that will attempt to lex as many tokens as possible,
/// without discrimination and until the end of input.
///
/// Uses German-specific lexing that currently reuses the English lexing
/// logic but is structured to allow future German-specific tokenization
/// if needed.
#[derive(Clone, Copy)]
pub struct PlainGerman;

impl Parser for PlainGerman {
    fn parse(&self, source: &[char]) -> Vec<Token> {
        lex_with(source, lex_german_token)
    }

    /// Not English, so [`Document`](harper_core::Document) skips the Brill
    /// tagger and the neural chunker, which are English-only models.
    fn is_english(&self) -> bool {
        false
    }
}
