//! What the linters ask of a token's letters. One copy, so that they cannot drift apart.

use harper_core::{Document, Token};

/// The token's letters in lower case.
pub(crate) fn lowercase_of(token: &Token, document: &Document) -> String {
    document
        .get_span_content(&token.span)
        .iter()
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Does the token begin with a small letter?
pub(crate) fn starts_lowercase(token: &Token, document: &Document) -> bool {
    document
        .get_span_content(&token.span)
        .first()
        .is_some_and(|c| c.is_lowercase())
}

/// Does the token begin with a capital?
pub(crate) fn starts_uppercase(token: &Token, document: &Document) -> bool {
    document
        .get_span_content(&token.span)
        .first()
        .is_some_and(|c| c.is_uppercase())
}

/// Does `word` begin with a capital?
pub(crate) fn is_capitalized(word: &str) -> bool {
    word.chars().next().is_some_and(char::is_uppercase)
}
