use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Token, TokenKind, TokenStringExt};

use crate::grammar::noun_gender::NounGender;
use crate::morphology::GenderSet;

/// The finite verbs that most often stand second in a main clause. The same
/// list as `DassDas.weir`: auxiliaries, modals and the verbs a short statement
/// uses. Plural forms and infinitives are left out, because they also close an
/// elliptical subordinate clause.
const FINITE_VERBS: &[&str] = &[
    "ist", "war", "wäre", "wird", "würde", "hat", "hatte", "hätte", "kann", "konnte", "könnte",
    "muss", "musste", "müsste", "soll", "sollte", "darf", "durfte", "will", "wollte", "mag",
    "geht", "ging", "gibt", "gab", "klingt", "stimmt", "reicht", "passt", "macht", "heißt",
    "bedeutet", "zeigt", "liegt", "bringt", "kostet",
];

/// *Dass Wetter ist heute schlecht*: the conjunction written for the article.
///
/// `DassDas.weir` catches *dass* directly in front of a finite verb, where it
/// can only be the pronoun. Here a noun stands in between. The conjunction
/// would put the clause's finite verb at the end, so a finite verb right after
/// the noun, with more of the sentence behind it, means the clause is a main
/// clause and *Dass Wetter* is the article and its noun.
///
/// The noun has to be neuter, or *das* would be the wrong article too: *dass
/// Liebe ist schön* is a mistake, but not this one. A name is no noun, so
/// *dass Anna geht und Tom bleibt* stays untouched; so does every clause whose
/// verb stands at the end (*Dass Wetter umschlägt, ist klar*), because the
/// token after the verb must be a word.
pub struct GermanDassArticle {
    nouns: NounGender,
}

impl Default for GermanDassArticle {
    fn default() -> Self {
        Self::new()
    }
}

impl GermanDassArticle {
    pub fn new() -> Self {
        Self {
            nouns: NounGender::new(),
        }
    }

    fn text(token: &Token, document: &Document) -> String {
        document.get_span_content(&token.span).iter().collect()
    }

    fn is_neuter_noun(&self, token: &Token, document: &Document) -> bool {
        let text = Self::text(token, document);
        text.chars().next().is_some_and(char::is_uppercase)
            && self.nouns.genders(&text).contains(GenderSet::NEUTER)
    }
}

impl Linter for GermanDassArticle {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();

        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();

            for window in words.windows(4) {
                let [dass, noun, verb, after] = window else {
                    continue;
                };
                let conjunction = Self::text(dass, document);
                if conjunction.to_lowercase() != "dass"
                    || !matches!(after.kind, TokenKind::Word(_))
                    || !FINITE_VERBS.contains(&Self::text(verb, document).as_str())
                    || !self.is_neuter_noun(noun, document)
                {
                    continue;
                }

                let fixed = if conjunction.starts_with('D') {
                    "Das"
                } else {
                    "das"
                };
                lints.push(Lint {
                    span: dass.span,
                    lint_kind: LintKind::Grammar,
                    suggestions: vec![Suggestion::ReplaceWith(fixed.chars().collect())],
                    message: format!(
                        "Hinter »{conjunction} {noun}« steht das Verb an zweiter Stelle, also ist \
                         das ein Hauptsatz. Gemeint ist der Artikel »{fixed}«.",
                        noun = Self::text(noun, document)
                    ),
                    priority: 31,
                });
            }
        }

        lints
    }

    fn description(&self) -> &str {
        "Ersetzt »dass« durch den Artikel »das«, wenn ein sächliches Nomen und gleich danach das Verb folgen."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanDassArticle;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::{Linter, Suggestion};

    fn fixes(text: &str) -> Vec<String> {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanDassArticle::new()
            .lint(&document)
            .into_iter()
            .map(|lint| {
                let mut chars: Vec<char> = text.chars().collect();
                if let Some(Suggestion::ReplaceWith(with)) = lint.suggestions.first() {
                    chars.splice(lint.span.start..lint.span.end, with.iter().copied());
                }
                chars.into_iter().collect()
            })
            .collect()
    }

    #[test]
    fn corrects_dass_used_as_the_article() {
        for (wrong, right) in [
            (
                "Dass Wetter ist heute schlecht.",
                "Das Wetter ist heute schlecht.",
            ),
            ("Dass Haus hat drei Zimmer.", "Das Haus hat drei Zimmer."),
            ("Dass Kind kann schon lesen.", "Das Kind kann schon lesen."),
            (
                "Ich glaube, dass Auto war zu teuer.",
                "Ich glaube, das Auto war zu teuer.",
            ),
            (
                "Dass Problem liegt woanders.",
                "Das Problem liegt woanders.",
            ),
            (
                "Er meint, dass Essen wird kalt.",
                "Er meint, das Essen wird kalt.",
            ),
        ] {
            assert_eq!(fixes(wrong), vec![right.to_owned()], "{wrong}");
        }
    }

    #[test]
    fn leaves_the_conjunction_alone() {
        for text in [
            "Dass er kommt, ist klar.",
            "Ich weiß, dass das Wetter schlecht ist.",
            "Dass Wetter umschlägt, ist bekannt.",
            "Es ist klar, dass Wasser nass ist.",
            "Ich weiß, dass Anna geht und Tom bleibt.",
            "Dass Liebe ist schön, sagt jeder.",
            "Ich hoffe, dass Geld reicht.",
            "Das Wetter ist heute schlecht.",
            "Er sagt, dass Peter ist krank.",
        ] {
            assert!(fixes(text).is_empty(), "{text}");
        }
    }
}
