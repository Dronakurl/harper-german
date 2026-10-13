//! Entries mined from English quotations must not hide a German typo.
//!
//! `tire` sat in the dictionary as a lower-case noun, left over from a quoted
//! English title. Matching is case-insensitive, so `Tire` — the commonest typo
//! for `Tiere`, and one every school text produces — passed as a word.

#[cfg(test)]
mod tests {
    use crate::dialects::GermanDialect;
    use crate::linting::new_curated_german;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::{LintKind, Linter};

    fn misspelled(text: &str) -> Vec<String> {
        let dict = curated_german_dictionary();
        let mut linter = new_curated_german(GermanDialect::Standard, dict.clone());
        let document = Document::new(text, &PlainGerman, &dict);
        linter
            .lint(&document)
            .into_iter()
            .filter(|lint| lint.lint_kind == LintKind::Spelling)
            .map(|lint| document.get_span_content_str(&lint.span))
            .collect()
    }

    #[test]
    fn tire_for_tiere_is_reported() {
        assert_eq!(misspelled("Wir haben viele Tiee gesehen."), ["Tiee"]);
    }

    #[test]
    fn tiere_is_not_reported() {
        assert!(misspelled("Wir haben viele Tiere gesehen.").is_empty());
    }
}
