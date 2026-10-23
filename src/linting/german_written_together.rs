use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Span, TokenKind};

/// Articles that form one word with *selbe*: *dieselbe*, *denselben*, *desselben*.
const SELB_ARTICLES: &[&str] = &["der", "die", "das", "den", "dem", "des"];

/// The forms of *selb-* that follow them. *Im selben*, *am selben* and *zur selben* are
/// contractions and correct apart, so they are not articles here.
const SELB_FORMS: &[&str] = &["selbe", "selben"];

/// Prepositions that form one word with *einander*: *miteinander*, *voneinander*.
const EINANDER_PREPOSITIONS: &[&str] = &[
    "an", "auf", "aus", "bei", "durch", "für", "gegen", "hinter", "in", "mit", "nach", "neben",
    "über", "um", "unter", "von", "vor", "zu",
];

/// Words written together that belong apart, and apart that belong together.
///
/// Both halves are words in their own right, so the spell checker has nothing to say: *mit
/// einander* is two correct words, *wieviele* a compound of two. Only closed lists are
/// consulted:
///
/// * an article and *selbe(n)*: *die selben Rechte* → *dieselben*;
/// * a preposition and *einander*: *mit einander* → *miteinander*;
/// * *wieviel* and its forms, closed up before 1996: *wieviele* → *wie viele*. The ordinal
///   *wievielte* is one word and stays.
#[derive(Default)]
pub struct GermanWrittenTogether;

fn lowercase(chars: &[char]) -> String {
    chars.iter().flat_map(|c| c.to_lowercase()).collect()
}

/// `text` with the first letter of `like`'s case.
fn in_case_of(text: &str, like: &[char]) -> Vec<char> {
    let mut chars: Vec<char> = text.chars().collect();
    if like.first().is_some_and(|c| c.is_uppercase())
        && let Some(first) = chars.first_mut()
    {
        *first = first.to_uppercase().next().unwrap_or(*first);
    }
    chars
}

impl Linter for GermanWrittenTogether {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let tokens = document.get_tokens();
        let mut lints = Vec::new();

        for (at, token) in tokens.iter().enumerate() {
            if !matches!(token.kind, TokenKind::Word(_)) {
                continue;
            }
            let first = document.get_span_content(&token.span);
            let first_lower = lowercase(first);

            // *wieviele* -> *wie viele*
            if let Some(ending) = first_lower.strip_prefix("wieviel")
                && ["", "e", "en", "er", "em", "es"].contains(&ending)
            {
                lints.push(Lint {
                    span: token.span,
                    lint_kind: LintKind::Spelling,
                    suggestions: vec![Suggestion::ReplaceWith(in_case_of(
                        &format!("wie viel{ending}"),
                        first,
                    ))],
                    message: "»wie viel« wird seit 1996 getrennt geschrieben.".to_owned(),
                    priority: 31,
                });
                continue;
            }

            // A single space, then the second word.
            let (Some(space), Some(next)) = (tokens.get(at + 1), tokens.get(at + 2)) else {
                continue;
            };
            if !matches!(space.kind, TokenKind::Space(1))
                || !matches!(next.kind, TokenKind::Word(_))
            {
                continue;
            }
            let second = document.get_span_content(&next.span);
            // The second half in lower case only: *Die Selben* may be a name.
            if second.first().is_some_and(|c| c.is_uppercase()) {
                continue;
            }
            let second_lower = lowercase(second);

            let joins = (SELB_ARTICLES.contains(&first_lower.as_str())
                && SELB_FORMS.contains(&second_lower.as_str()))
                || (EINANDER_PREPOSITIONS.contains(&first_lower.as_str())
                    && second_lower == "einander");
            if !joins {
                continue;
            }

            let joined = in_case_of(&format!("{first_lower}{second_lower}"), first);
            let shown: String = joined.iter().collect();
            lints.push(Lint {
                span: Span::new(token.span.start, next.span.end),
                lint_kind: LintKind::Spelling,
                suggestions: vec![Suggestion::ReplaceWith(joined)],
                message: format!("»{shown}« wird zusammengeschrieben."),
                priority: 31,
            });
        }

        lints
    }

    fn description(&self) -> &str {
        "Schreibt »dieselben«, »miteinander« und Verwandte zusammen und »wie viele« getrennt."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanWrittenTogether;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::{Linter, Suggestion};

    fn fixes(text: &str) -> Vec<String> {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanWrittenTogether
            .lint(&document)
            .into_iter()
            .map(|lint| match &lint.suggestions[0] {
                Suggestion::ReplaceWith(chars) => chars.iter().collect(),
                other => panic!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn joins_what_belongs_together() {
        assert_eq!(fixes("Die Frauen wollen die selben Rechte."), ["dieselben"]);
        assert_eq!(fixes("Das ist der selbe Fehler."), ["derselbe"]);
        assert_eq!(fixes("Das gilt des selben Jahres."), ["desselben"]);
        assert_eq!(fixes("Sie reden mit einander."), ["miteinander"]);
        assert_eq!(
            fixes("Wir lernen von einander und für einander."),
            ["voneinander", "füreinander"]
        );
        assert_eq!(fixes("Mit einander geht es besser."), ["Miteinander"]);
    }

    #[test]
    fn splits_wieviel() {
        assert_eq!(fixes("Wieviele Jahre dauert das?"), ["Wie viele"]);
        assert_eq!(fixes("Ich weiß nicht, wieviel es kostet."), ["wie viel"]);
        assert_eq!(fixes("Mit wievielen Leuten?"), ["wie vielen"]);
    }

    #[test]
    fn leaves_the_correct_spellings_alone() {
        for text in [
            "Sie wollen dieselben Rechte und reden miteinander.",
            "Wir trafen uns im selben Moment am selben Ort.",
            "Der wievielte Tag ist heute?",
            "Wie viele Jahre dauert das?",
            "Die Selben sind eine Band.",
        ] {
            assert!(fixes(text).is_empty(), "{text}: {:?}", fixes(text));
        }
    }
}
