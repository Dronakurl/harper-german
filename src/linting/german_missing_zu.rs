//! *um ein Geschenk kaufen*, *Wir haben vor, nach Italien fahren*: an infinitive group without
//! its *zu*.

use harper_core::{
    Document, Punctuation, Token, TokenKind, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
};

use crate::grammar::noun_phrase::lowercase_of;
use crate::grammar::token_text::starts_lowercase;
use crate::grammar::verbs::{INFINITIVE_GROUP_OPENERS, joined_separable_verb};
use crate::linting::german_relative_clause_comma::GermanRelativeClauseComma;
use crate::spell::curated_german_dictionary;
use harper_core::spell::Dictionary;

/// The separable prefixes that take the *zu* inside the word (*einzukaufen*), longest first.
const PREFIXES: &[&str] = &[
    "zurück", "zusammen", "vorbei", "kennen", "heim", "fern", "frei", "fest", "teil", "weg", "auf",
    "aus", "ein", "mit", "vor", "nach", "her", "hin", "los", "dar", "bei", "an", "ab",
];

/// The stems of verbs that take an infinitive group with *zu* after a comma: *Ich versuche,
/// pünktlich zu sein*, *Wir haben vor, nach Italien zu fahren*.
const ZU_GOVERNORS: &[&str] = &[
    "versuch",
    "beschließ",
    "beschloss",
    "vergess",
    "vergaß",
    "vergiss",
    "hoff",
    "plan",
    "beginn",
    "begann",
    "versprech",
    "versprach",
    "versproch",
    "empfehl",
    "empfahl",
    "empfohl",
    "erlaub",
    "verbiet",
    "verbot",
    "gebeten",
];

/// Forms of the finite verbs that make a clause of their own: *…, musste angehalten werden*.
const FINITE_FORMS: &[&str] = &[
    "muss", "musste", "mussten", "kann", "konnte", "konnten", "soll", "sollte", "sollten", "will",
    "wollte", "wollten", "darf", "durfte", "durften", "wird", "wurde", "wurden", "ist", "war",
    "waren", "sind", "hat", "hatte", "hatten",
];

fn is_clause_break(token: &Token) -> bool {
    matches!(
        token.kind,
        TokenKind::Punctuation(
            Punctuation::Comma
                | Punctuation::Period
                | Punctuation::Bang
                | Punctuation::Question
                | Punctuation::Semicolon
                | Punctuation::Colon
        )
    )
}

/// Reports an infinitive that closes an infinitive group without its *zu*.
///
/// Two kinds of group are read, each a clause of its own between commas or the end of the
/// sentence, with the infinitive as its last word and no other verb in it:
///
/// * one that opens with *um*, *ohne*, *statt* or *anstatt*, behind a comma or, at the start of
///   the sentence, ended by one (*Um zu gewinnen, …*); not *ohne dass*;
/// * one behind a comma after a verb that governs a *zu*-infinitive: *versuchen*, *vorhaben*
///   (*habe … vor*), *beschließen*, *vergessen*, *hoffen*, *planen*, *beginnen*.
#[derive(Default)]
pub struct GermanMissingZu;

impl GermanMissingZu {
    fn is_infinitive(token: &Token, document: &Document) -> bool {
        let word = lowercase_of(token, document);
        starts_lowercase(token, document)
            && word.ends_with('n')
            && word.chars().count() > 3
            && (GermanRelativeClauseComma::is_verb_token(token, document)
                || GermanRelativeClauseComma::infinitive_is_a_verb(&word))
    }

    /// Does the group already have its *zu*, as a word or inside the infinitive (*einzukaufen*,
    /// *sicherzustellen*, *auseinanderzusetzen*)?
    fn has_zu(group: &[&Token], document: &Document) -> bool {
        group.iter().any(|token| {
            let word = lowercase_of(token, document);
            word == "zu"
                || word.match_indices("zu").any(|(at, _)| {
                    at >= 2
                        && word.len() - at - 2 >= 3
                        && curated_german_dictionary()
                            .get_word_metadata(
                                &format!("{}{}", &word[..at], &word[at + 2..])
                                    .chars()
                                    .collect::<Vec<_>>(),
                            )
                            .is_some_and(|metadata| metadata.is_verb())
                })
        })
    }

    /// The infinitive with its *zu*: *kaufen* → *zu kaufen*, *einkaufen* → *einzukaufen*.
    fn with_zu(infinitive: &str) -> String {
        for prefix in PREFIXES {
            if let Some(rest) = infinitive.strip_prefix(prefix)
                && rest.chars().count() >= 3
                && joined_separable_verb(prefix, rest).is_some()
            {
                return format!("{prefix}zu{rest}");
            }
        }
        format!("zu {infinitive}")
    }

    fn report(token: &Token, document: &Document) -> Lint {
        let infinitive = lowercase_of(token, document);
        let fixed = Self::with_zu(&infinitive);
        Lint {
            span: token.span,
            lint_kind: LintKind::Grammar,
            suggestions: vec![Suggestion::ReplaceWith(fixed.chars().collect())],
            message: format!("Vor dem Infinitiv der Gruppe fehlt »zu«: »{fixed}«."),
            priority: 31,
        }
    }
}

impl Linter for GermanMissingZu {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            // Clauses as (start, end, ended by a comma).
            let mut clauses = Vec::new();
            let mut start = 0;
            for (at, token) in words.iter().enumerate() {
                if is_clause_break(token) {
                    clauses.push((start, at, token.kind.is_comma()));
                    start = at + 1;
                }
            }
            if start < words.len() {
                clauses.push((start, words.len(), false));
            }

            for (index, &(start, end, ended_by_comma)) in clauses.iter().enumerate() {
                if end <= start + 1 {
                    continue;
                }
                let group = &words[start..end];
                let last = group[group.len() - 1];
                if !Self::is_infinitive(last, document) || Self::has_zu(group, document) {
                    continue;
                }
                // No other verb in the group: *Um 8 Uhr gehen wir schlafen* has one.
                let other_verb = group[..group.len() - 1].iter().skip(1).any(|token| {
                    starts_lowercase(token, document)
                        && GermanRelativeClauseComma::is_verb_token(token, document)
                        && !token.kind.is_adjective()
                        && !token.kind.is_determiner()
                        && !token.kind.is_pronoun()
                        && !token.kind.is_preposition()
                });
                if other_verb {
                    continue;
                }
                let first = lowercase_of(group[0], document);
                let after_comma = start > 0 && words[start - 1].kind.is_comma();

                let opened = INFINITIVE_GROUP_OPENERS.contains(&first.as_str())
                    && group
                        .get(1)
                        .is_some_and(|next| lowercase_of(next, document) != "dass")
                    && (after_comma || (start == 0 && ended_by_comma));

                // *Ich versuche, pünktlich sein*: the clause in front has a verb that takes
                // a *zu*-infinitive, and this one has no subject of its own.
                let governed = after_comma
                    && index > 0
                    && {
                        let (previous_start, previous_end, _) = clauses[index - 1];
                        // Lower-case words only: *ihr Plan*, *das Versprechen* are nouns.
                        let previous: Vec<String> = words[previous_start..previous_end]
                            .iter()
                            .filter(|token| starts_lowercase(token, document))
                            .map(|token| lowercase_of(token, document))
                            .collect();
                        let governs = previous.iter().any(|word| {
                            ZU_GOVERNORS.iter().any(|stem| word.starts_with(stem))
                                || ["bat", "baten", "bitte", "bittet"].contains(&word.as_str())
                        }) || (previous.last().is_some_and(|word| word == "vor")
                            && previous.iter().any(|word| {
                                ["habe", "hast", "hat", "haben", "habt", "hatte", "hatten"]
                                    .contains(&word.as_str())
                            }));
                        governs
                    }
                    && !group.iter().any(|token| {
                        token.kind.is_pronoun()
                            && matches!(
                                lowercase_of(token, document).as_str(),
                                "ich" | "du" | "er" | "sie" | "es" | "wir" | "ihr" | "man"
                            )
                    })
                    && !matches!(
                        first.as_str(),
                        "dass"
                            | "ob"
                            | "wenn"
                            | "weil"
                            | "wie"
                            | "was"
                            | "wo"
                            | "wann"
                            | "warum"
                            | "und"
                            | "oder"
                            | "aber"
                            | "denn"
                            | "als"
                            | "damit"
                    );

                // A subject or a relative pronoun makes it a clause with a finite verb: *um den
                // wir uns kümmerten*, *die alle paar Monate stattfinden*.
                let finite_clause = group.iter().any(|token| {
                    matches!(
                        lowercase_of(token, document).as_str(),
                        "ich"
                            | "du"
                            | "er"
                            | "sie"
                            | "es"
                            | "wir"
                            | "ihr"
                            | "man"
                            | "welche"
                            | "welcher"
                            | "welches"
                            | "welchen"
                            | "welchem"
                    ) || FINITE_FORMS.contains(&lowercase_of(token, document).as_str())
                }) || matches!(
                    first.as_str(),
                    "der"
                        | "die"
                        | "das"
                        | "den"
                        | "dem"
                        | "deren"
                        | "dessen"
                        | "da"
                        | "wo"
                        | "sollte"
                        | "sollten"
                ) || (INFINITIVE_GROUP_OPENERS.contains(&first.as_str())
                    && group.get(1).is_some_and(|next| {
                        matches!(
                            lowercase_of(next, document).as_str(),
                            "den" | "die" | "das" | "dem" | "der" | "welchen" | "welche" | "was"
                        ) && group.len() > 2
                            && group[2..].iter().any(|t| t.kind.is_pronoun())
                    }));
                if (opened || governed) && !finite_clause {
                    lints.push(Self::report(last, document));
                }
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Ergänzt das fehlende »zu« in Infinitivgruppen (»um ein Geschenk zu kaufen«)."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanMissingZu;
    use crate::parsers::PlainGerman;
    use crate::spell::curated_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn fixes(text: &str) -> Vec<String> {
        let dict = curated_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanMissingZu
            .lint(&document)
            .into_iter()
            .flat_map(|lint| lint.suggestions)
            .map(|suggestion| suggestion.to_string())
            .collect()
    }

    #[test]
    fn an_infinitive_group_without_zu_is_reported() {
        for (text, fixed) in [
            (
                "Ich gehe in die Stadt, um ein Geschenk kaufen.",
                "zu kaufen",
            ),
            ("Wir haben vor, nach Italien fahren.", "zu fahren"),
            ("Ich versuche, pünktlich kommen.", "zu kommen"),
            ("Er ging, ohne ein Wort sagen.", "zu sagen"),
            ("Wir fahren in die Stadt, um einkaufen.", "einzukaufen"),
        ] {
            assert_eq!(fixes(text), [format!("Replace with: “{fixed}”")], "{text}");
        }
    }

    #[test]
    fn groups_with_zu_and_other_clauses_are_quiet() {
        for text in [
            "Ich gehe in die Stadt, um ein Geschenk zu kaufen.",
            "Wir fahren in die Stadt, um einzukaufen.",
            "Um 8 Uhr gehen wir schlafen.",
            "Er ging, ohne dass wir es merkten.",
            "Ich versuche, dass wir kommen.",
            "Ich hoffe, wir sehen uns.",
            "Statt Kaffee trinken wir Tee.",
            "Wir haben vor, dass er kommt.",
            "Ich versuche, ob es geht.",
            "Er hofft, sie kommen.",
            "Wir empfehlen die Weinproben, die alle paar Monate stattfinden.",
            "Er tat es, um sicherzustellen, dass alles klappt.",
            "Sie verrät ihren Plan, ein Buch herauszugeben.",
            "Niemand, um den wir uns kümmerten, wurde verletzt.",
            "Er hoffte, die Umwandlung fortzusetzen.",
        ] {
            assert!(fixes(text).is_empty(), "{text}: {:?}", fixes(text));
        }
    }
}
