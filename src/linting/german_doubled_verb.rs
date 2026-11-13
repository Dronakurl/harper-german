//! *Ich denke, dass er kommen wird werden*, *Er hat sich verletzt gehabt*, *Er ist gegangen
//! worden*: a verb at the end of the clause that the clause already has, or that cannot follow.

use harper_core::{
    Document, Punctuation, Span, Token, TokenKind, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
};

use crate::grammar::noun_phrase::lowercase_of;

const WERDEN: &[&str] = &["werde", "wirst", "wird", "werden", "werdet"];
const WUERDE: &[&str] = &["würde", "würdest", "würden", "würdet"];
const HABEN: &[&str] = &["habe", "hast", "hat", "haben", "habt", "hatte", "hatten"];
const SEIN: &[&str] = &["bin", "bist", "ist", "sind", "seid", "war", "waren"];

/// Participles of verbs that form the perfect with *sein*: a passive *worden* cannot follow
/// them.
const SEIN_PARTICIPLES: &[&str] = &[
    "gegangen",
    "gekommen",
    "gefahren",
    "gelaufen",
    "geflogen",
    "gestorben",
    "geblieben",
    "gewesen",
    "geworden",
    "gefallen",
    "gereist",
    "geschwommen",
    "angekommen",
    "abgefahren",
];

fn is_clause_end(token: Option<&&Token>) -> bool {
    token.is_none_or(|t| {
        matches!(
            t.kind,
            TokenKind::Punctuation(
                Punctuation::Period
                    | Punctuation::Comma
                    | Punctuation::Bang
                    | Punctuation::Question
                    | Punctuation::Semicolon
            )
        )
    })
}

/// Reports the last verb of a clause when the clause has it already or the verb before it
/// excludes it, and offers to remove it:
///
/// * *kommen wird werden*: a form of *werden* behind another one;
/// * *helfen würden* in a clause that has *würde* already, *hätte würde*;
/// * *verletzt gehabt* with *haben* earlier in the clause, the double perfect;
/// * *gegangen worden*: a passive of a verb that takes *sein*.
#[derive(Default)]
pub struct GermanDoubledVerb;

impl GermanDoubledVerb {
    fn remove(previous: &Token, extra: &Token, message: &str) -> Lint {
        Lint {
            span: Span::new(previous.span.end, extra.span.end),
            lint_kind: LintKind::Repetition,
            suggestions: vec![Suggestion::Remove],
            message: message.to_owned(),
            priority: 31,
        }
    }
}

impl Linter for GermanDoubledVerb {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            let mut start = 0;
            for at in 0..words.len() {
                if matches!(words[at].kind, TokenKind::Punctuation(Punctuation::Comma)) {
                    start = at + 1;
                    continue;
                }
                if at < start + 2 || !is_clause_end(words.get(at + 1)) {
                    continue;
                }
                let last = lowercase_of(words[at], document);
                let previous = lowercase_of(words[at - 1], document);
                let earlier: Vec<String> = words[start..at - 1]
                    .iter()
                    .map(|t| lowercase_of(t, document))
                    .collect();
                let message = if WERDEN.contains(&last.as_str())
                    && WERDEN.contains(&previous.as_str())
                {
                    Some("Hier steht »werden« zweimal.")
                } else if WUERDE.contains(&last.as_str())
                    && (earlier.iter().any(|w| WUERDE.contains(&w.as_str()))
                        || matches!(previous.as_str(), "hätte" | "wäre" | "hätten" | "wären"))
                {
                    Some("Der Konjunktiv steht schon im Satz; das zweite »würde« fällt weg.")
                } else if last == "gehabt"
                    && words[at - 1].kind.is_verb()
                    && (previous.starts_with("ge") || previous.ends_with('t'))
                    && previous != "gut"
                    && earlier.iter().any(|w| HABEN.contains(&w.as_str()))
                {
                    Some(
                        "Das doppelte Perfekt (»hat … verletzt gehabt«) ist umgangssprachlich; »gehabt« fällt weg.",
                    )
                } else if last == "gewesen"
                    && SEIN_PARTICIPLES.contains(&previous.as_str())
                    && earlier.iter().any(|w| SEIN.contains(&w.as_str()))
                {
                    Some(
                        "Das doppelte Perfekt (»ist … gegangen gewesen«) ist umgangssprachlich; »gewesen« fällt weg.",
                    )
                } else if last == "worden" && SEIN_PARTICIPLES.contains(&previous.as_str()) {
                    Some("Von diesem Verb gibt es kein Passiv; »worden« fällt weg.")
                } else {
                    None
                };
                if let Some(message) = message {
                    lints.push(Self::remove(words[at - 1], words[at], message));
                }
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Findet ein überzähliges Verb am Satzende (»kommen wird werden«, »verletzt gehabt«)."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanDoubledVerb;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn count(text: &str) -> usize {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanDoubledVerb.lint(&document).len()
    }

    #[test]
    fn a_doubled_verb_at_the_end_is_reported() {
        for text in [
            "Ich denke, dass er morgen kommen wird werden.",
            "Wenn ich Zeit hätte, würde ich dir helfen würden.",
            "Ich würde gerne kommen, wenn ich Zeit hätte würde.",
            "Er hat sich gestern beim Sport verletzt gehabt.",
            "Ich habe das Auto gestern repariert gehabt.",
            "Er ist ins Krankenhaus gegangen worden.",
        ] {
            assert_eq!(count(text), 1, "{text}");
        }
        for text in [
            "Ich denke, dass er morgen kommen wird.",
            "Er hätte kommen sollen.",
            "Er hat Glück gehabt.",
            "Das Haus ist verkauft worden.",
            "Ich habe keine Zeit gehabt.",
            "Wir haben es gut gehabt.",
            "Er hat immer Pech gehabt.",
            "Er würde kommen, wenn er könnte.",
            "Sie wird gewählt werden.",
        ] {
            assert_eq!(count(text), 0, "{text}");
        }
    }
}
