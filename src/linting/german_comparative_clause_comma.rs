//! Two short clauses that need commas the other comma rules do not see, because neither starts
//! with a conjunction of their lists: *je mehr ich lerne, desto mehr weiß ich* and *er lief, so
//! schnell er konnte, nach Hause*.

use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Punctuation, Token, TokenKind, TokenStringExt};

use crate::grammar::token_text::{lowercase_of, starts_lowercase};

const SUBJECT_PRONOUNS: &[&str] = &["ich", "du", "er", "sie", "es", "wir", "ihr", "man"];

/// The modal verbs that close *so schnell er konnte*, *so gut ich kann*, *so oft sie will*.
const MODALS: &[&str] = &[
    "kann",
    "kannst",
    "könnt",
    "können",
    "konnte",
    "konntest",
    "konnten",
    "könnte",
    "könnten",
    "will",
    "willst",
    "wollt",
    "wollen",
    "wollte",
    "wolltest",
    "wollten",
    "mag",
    "magst",
    "mochte",
    "möchte",
    "möchtest",
    "darf",
    "darfst",
    "durfte",
    "durften",
    "muss",
    "musst",
    "musste",
    "mussten",
    "soll",
    "sollst",
    "sollte",
    "sollten",
];

/// Reports the missing comma before *desto* or *umso* when the *je* part has a verb, and the
/// missing commas around *so* + adjective + pronoun + modal verb.
#[derive(Default)]
pub struct GermanComparativeClauseComma;

fn is_comma(token: &Token) -> bool {
    matches!(token.kind, TokenKind::Punctuation(Punctuation::Comma))
}

fn comma_after(token: &Token, message: &str) -> Lint {
    Lint {
        span: token.span,
        lint_kind: LintKind::Punctuation,
        suggestions: vec![Suggestion::InsertAfter(vec![','])],
        priority: 30,
        message: message.to_string(),
    }
}

/// *mehr*, *länger*, *besser*: a comparative, read from its spelling.
fn is_comparative(word: &str) -> bool {
    matches!(
        word,
        "mehr" | "weniger" | "eher" | "lieber" | "besser" | "öfter"
    ) || (word.ends_with("er") && word.chars().count() > 4)
}

impl GermanComparativeClauseComma {
    fn je_desto(words: &[&Token], document: &Document, lints: &mut Vec<Lint>) {
        for (at, je) in words.iter().enumerate() {
            if lowercase_of(je, document) != "je"
                || !words
                    .get(at + 1)
                    .is_some_and(|w| is_comparative(&lowercase_of(w, document)))
            {
                continue;
            }
            let mut verb_seen = false;
            for i in at + 2..words.len() {
                let word = words[i];
                if is_comma(word) || word.kind.is_sentence_terminator() {
                    break;
                }
                let lower = lowercase_of(word, document);
                if lower == "desto" || lower == "umso" {
                    if verb_seen {
                        lints.push(comma_after(
                            words[i - 1],
                            "Vor »desto« und »umso« steht ein Komma, wenn der »je«-Teil ein Verb hat.",
                        ));
                    }
                    break;
                }
                verb_seen |= word.kind.is_verb() && starts_lowercase(word, document);
            }
        }
    }

    fn so_adjective_clause(words: &[&Token], document: &Document, lints: &mut Vec<Lint>) {
        const MESSAGE: &str =
            "»so schnell er konnte« ist ein verkürzter Nebensatz und steht zwischen Kommas.";
        for at in 0..words.len().saturating_sub(3) {
            let [so, adjective, pronoun, modal] =
                [words[at], words[at + 1], words[at + 2], words[at + 3]];
            if lowercase_of(so, document) != "so"
                || !(adjective.kind.is_adjective() || adjective.kind.is_adverb())
                || !starts_lowercase(adjective, document)
                || !SUBJECT_PRONOUNS.contains(&lowercase_of(pronoun, document).as_str())
                || !MODALS.contains(&lowercase_of(modal, document).as_str())
            {
                continue;
            }
            if let Some(before) = at.checked_sub(1).map(|i| words[i])
                && before.kind.is_word()
            {
                lints.push(comma_after(before, MESSAGE));
            }
            if words.get(at + 4).is_some_and(|after| after.kind.is_word()) {
                lints.push(comma_after(modal, MESSAGE));
            }
        }
    }
}

impl Linter for GermanComparativeClauseComma {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|t| !t.kind.is_whitespace())
                .collect();
            Self::je_desto(&words, document, &mut lints);
            Self::so_adjective_clause(&words, document, &mut lints);
        }
        lints
    }

    fn description(&self) -> &str {
        "Setzt das Komma vor »desto« und um »so schnell er konnte«."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanComparativeClauseComma;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn fixed(text: &str) -> String {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        let mut lints = GermanComparativeClauseComma.lint(&document);
        lints.sort_by_key(|lint| std::cmp::Reverse(lint.span.start));
        let mut chars: Vec<char> = text.chars().collect();
        for lint in lints {
            lint.suggestions[0].apply(lint.span, &mut chars);
        }
        chars.into_iter().collect()
    }

    #[test]
    fn the_commas_are_added() {
        for (text, expected) in [
            (
                "Je mehr ich lerne desto mehr weiß ich.",
                "Je mehr ich lerne, desto mehr weiß ich.",
            ),
            (
                "Je länger ich warte desto ungeduldiger werde ich.",
                "Je länger ich warte, desto ungeduldiger werde ich.",
            ),
            (
                "Er lief so schnell er konnte.",
                "Er lief, so schnell er konnte.",
            ),
            (
                "Er rannte so schnell er konnte nach Hause.",
                "Er rannte, so schnell er konnte, nach Hause.",
            ),
        ] {
            assert_eq!(fixed(text), expected);
        }
    }

    #[test]
    fn correct_sentences_stay() {
        for text in [
            "Je mehr, desto besser.",
            "Je nachdem, wie es läuft.",
            "Je mehr ich lerne, desto mehr weiß ich.",
            "Er lief so schnell wie nie.",
            "Er lief so schnell, dass er fiel.",
            "Er lief, so schnell er konnte, nach Hause.",
        ] {
            assert_eq!(fixed(text), text);
        }
    }
}
