//! Commas that the coordinating conjunctions and a greeting require.

use crate::grammar::subjects::irregular_finite_verb;
use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Punctuation, Token, TokenKind, TokenStringExt};

/// Pronouns that open a main clause of their own behind *aber* or *denn*.
const SUBJECT_PRONOUNS: &[&str] = &["ich", "du", "er", "sie", "es", "wir", "ihr", "man"];

/// What a greeting may be followed by, behind the name, to need the comma:
/// *Hallo Peter**,** wie geht es dir?*. A short, closed list; a greeting and a name followed
/// by anything else is left alone.
const AFTER_THE_NAME: &[&str] = &[
    "wie",
    "ich",
    "wir",
    "vielen",
    "danke",
    "bitte",
    "hast",
    "hättest",
    "kannst",
    "könntest",
    "schön",
    "herzlich",
    "anbei",
    "es",
    "heute",
];

/// What a formal salutation (*Sehr geehrte Frau Müller*) may be followed by, without its comma.
const AFTER_THE_SALUTATION: &[&str] = &[
    "vielen",
    "ich",
    "wir",
    "hiermit",
    "anbei",
    "danke",
    "bezugnehmend",
    "herzlichen",
    "besten",
    "leider",
    "gerne",
    "zunächst",
    "aufgrund",
    "bitte",
    "unter",
    "im",
    "für",
];

const GREETINGS: &[&str] = &[
    "hallo", "hi", "hey", "moin", "servus", "liebe", "lieber", "liebes",
];

/// Requires the comma in front of *sondern*, in front of *aber* and *denn* when a main clause
/// follows, and behind a greeting and a name.
///
/// * *sondern* is a conjunction, and it takes a comma every time (Duden, § 71): *nicht heute**,**
///   sondern morgen*, *nicht nur A**,** sondern auch B*. Except as the verb *absondern*, whose
///   particle *ab* gives it away.
/// * *aber* and *denn* are also particles inside a clause (*Das ist aber schön*, *Wo bist du
///   denn?*), where a comma would be wrong. Only the clause-opening use is checked, which shows
///   itself by a subject pronoun right behind it; and the clause in front has to be complete,
///   with a finite verb that is not the word right before the conjunction. That keeps out the
///   inverted particle (*Damals wollte aber er nichts davon wissen*), where the verb comes first.
/// * *Hallo Peter wie geht es dir?*
///
/// Not checked: *aber* in front of a phrase (*klein aber fein*), *doch* and *jedoch* (each also a
/// particle or adverb), and a greeting followed by anything but the closed list above.
#[derive(Default)]
pub struct GermanCoordinatingComma;

impl GermanCoordinatingComma {
    fn word(token: &Token, document: &Document) -> String {
        document
            .get_span_content(&token.span)
            .iter()
            .flat_map(|c| c.to_lowercase())
            .collect()
    }

    fn is_word(token: &Token) -> bool {
        matches!(token.kind, TokenKind::Word(_))
    }

    /// A form the dictionary or the table of irregular verbs reads as finite, not an infinitive
    /// or a participle. *-en* never counts: it is the infinitive and the plural at once.
    fn looks_finite(token: &Token, document: &Document) -> bool {
        let word = Self::word(token, document);
        if irregular_finite_verb(&word).is_some() {
            return true;
        }
        let lowercase = document
            .get_span_content(&token.span)
            .first()
            .is_some_and(|c| c.is_lowercase());
        lowercase
            && token.kind.is_verb()
            && !word.ends_with("en")
            && !word.starts_with("ge")
            && (word.ends_with("te")
                || word.ends_with("st")
                || word.ends_with('e')
                || word.ends_with('t'))
    }

    fn insert_comma_after(token: &Token, message: String) -> Lint {
        Lint {
            span: token.span,
            lint_kind: LintKind::Punctuation,
            suggestions: vec![Suggestion::InsertAfter(vec![','])],
            priority: 30,
            message,
        }
    }

    fn is_mention_or_list(tokens: &[&Token], index: usize) -> bool {
        // *das sondern-Wort*, or the conjunction named between brackets or quotes.
        tokens
            .get(index + 1)
            .is_some_and(|next| matches!(next.kind, TokenKind::Punctuation(Punctuation::Hyphen)))
    }
}

impl Linter for GermanCoordinatingComma {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();

        for sentence in document.iter_sentences() {
            let tokens: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();

            for index in 1..tokens.len() {
                let token = tokens[index];
                let previous = tokens[index - 1];
                if !Self::is_word(token) || !Self::is_word(previous) {
                    continue;
                }
                let is_lowercase = document
                    .get_span_content(&token.span)
                    .first()
                    .is_some_and(|c| c.is_lowercase());
                if !is_lowercase || Self::is_mention_or_list(&tokens, index) {
                    continue;
                }
                let word = Self::word(token, document);

                match word.as_str() {
                    "sondern" => {
                        // The verb *absondern* is separable: *Diese Insekten sondern einen
                        // Duftstoff ab*. The particle at the end of the clause gives it away, and
                        // it is the one case in which *sondern* is no conjunction.
                        if tokens[index + 1..]
                            .iter()
                            .any(|later| Self::word(later, document) == "ab")
                        {
                            continue;
                        }
                        // *nicht A und sondern B* is no German; a coordinator in front is not
                        // ours to touch.
                        if ["und", "oder"].contains(&Self::word(previous, document).as_str()) {
                            continue;
                        }
                        lints.push(Self::insert_comma_after(
                            previous,
                            "Vor »sondern« steht immer ein Komma.".to_owned(),
                        ));
                    }
                    "aber" | "denn" => {
                        let Some(next) = tokens.get(index + 1) else {
                            continue;
                        };
                        if !Self::is_word(next)
                            || !SUBJECT_PRONOUNS.contains(&Self::word(next, document).as_str())
                        {
                            continue;
                        }
                        // A finite verb earlier in the sentence, and not the word right before
                        // the conjunction, which would make this the inverted particle.
                        let finite_before = (0..index - 1).any(|at| {
                            Self::is_word(tokens[at]) && Self::looks_finite(tokens[at], document)
                        });
                        if !finite_before || Self::looks_finite(previous, document) {
                            continue;
                        }
                        lints.push(Self::insert_comma_after(
                            previous,
                            format!("Vor »{word}« steht ein Komma, wenn ein Hauptsatz folgt."),
                        ));
                    }
                    _ => {}
                }
            }

            // *Sehr geehrte Frau Müller vielen Dank*: the salutation is a run of capitalized words
            // (and *und*: *Damen und Herren*), then the letter goes on in small letters.
            if tokens.len() >= 4
                && Self::word(tokens[0], document) == "sehr"
                && ["geehrte", "geehrter", "geehrtes"]
                    .contains(&Self::word(tokens[1], document).as_str())
            {
                let capitalized = |token: &Token| {
                    Self::is_word(token)
                        && document
                            .get_span_content(&token.span)
                            .first()
                            .is_some_and(|c| c.is_uppercase())
                };
                let mut end = 2;
                while end < tokens.len()
                    && end < 8
                    && (capitalized(tokens[end]) || Self::word(tokens[end], document) == "und")
                {
                    end += 1;
                }
                if end >= 3
                    && end < tokens.len()
                    && capitalized(tokens[end - 1])
                    && Self::is_word(tokens[end])
                    && AFTER_THE_SALUTATION.contains(&Self::word(tokens[end], document).as_str())
                {
                    lints.push(Self::insert_comma_after(
                        tokens[end - 1],
                        "Nach der Anrede steht ein Komma.".to_owned(),
                    ));
                }
            }

            // Greeting, name, and then the rest of the sentence.
            if tokens.len() >= 3
                && Self::is_word(tokens[0])
                && Self::is_word(tokens[1])
                && Self::is_word(tokens[2])
                && GREETINGS.contains(&Self::word(tokens[0], document).as_str())
                && document
                    .get_span_content(&tokens[1].span)
                    .first()
                    .is_some_and(|c| c.is_uppercase())
                && AFTER_THE_NAME.contains(&Self::word(tokens[2], document).as_str())
                && !tokens[1].kind.is_determiner()
                && !tokens[1].kind.is_pronoun()
            {
                lints.push(Self::insert_comma_after(
                    tokens[1],
                    "Nach der Anrede steht ein Komma.".to_owned(),
                ));
            }
        }

        lints
    }

    fn description(&self) -> &str {
        "Setzt das Komma vor »sondern«, vor »aber« und »denn« vor einem Hauptsatz und nach Gruß und Name."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanCoordinatingComma;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn lint_count(text: &str) -> usize {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanCoordinatingComma.lint(&document).len()
    }

    #[test]
    fn sondern_always_takes_a_comma() {
        for text in [
            "Er kommt nicht heute sondern morgen.",
            "Das ist nicht nur teuer sondern auch schlecht.",
            "Sie wollte nicht gehen sondern bleiben.",
            "Es ist nicht Sommer sondern Winter.",
        ] {
            assert_eq!(lint_count(text), 1, "{text}");
        }
        for text in [
            "Er kommt nicht heute, sondern morgen.",
            "Sondern was dann?",
            "Das war nicht er, sondern sie.",
            "Diese Insekten sondern einen bestimmten Duftstoff ab.",
            "Sie sondern nicht einmal den Gestank ab.",
        ] {
            assert_eq!(lint_count(text), 0, "{text}");
        }
    }

    #[test]
    fn aber_and_denn_before_a_main_clause_take_a_comma() {
        for text in [
            "Ich wollte kommen aber ich hatte keine Zeit.",
            "Ich bleibe zu Hause denn ich bin krank.",
            "Wir haben lange gewartet aber er kam nicht.",
        ] {
            assert_eq!(lint_count(text), 1, "{text}");
        }
    }

    #[test]
    fn the_particle_and_the_phrase_are_left_alone() {
        for text in [
            "Ich wollte kommen, aber ich hatte keine Zeit.",
            "Das ist aber schön.",
            "Damals wollte aber er nichts davon wissen.",
            "Wo bist denn du?",
            "Der Kuchen ist klein aber fein.",
            "Aber ich habe keine Zeit.",
        ] {
            assert_eq!(lint_count(text), 0, "{text}");
        }
    }

    #[test]
    fn a_greeting_and_a_name_take_a_comma() {
        for text in [
            "Hallo Peter wie geht es dir?",
            "Liebe Anna ich hoffe es geht dir gut.",
            "Hi Tom danke für die Hilfe.",
            "Sehr geehrte Frau Müller vielen Dank für Ihre Nachricht.",
            "Sehr geehrte Damen und Herren hiermit bewerbe ich mich.",
            "Sehr geehrter Herr Schmidt ich schreibe Ihnen.",
        ] {
            assert_eq!(lint_count(text), 1, "{text}");
        }
        for text in [
            "Hallo Peter, wie geht es dir?",
            "Hallo Welt.",
            "Hallo Peter Müller kommt später.",
            "Sehr geehrte Frau Müller, vielen Dank für Ihre Nachricht.",
            "Sehr geehrte Damen und Herren, hiermit bewerbe ich mich.",
            "Sehr geehrte Kunden sind willkommen.",
        ] {
            assert_eq!(lint_count(text), 0, "{text}");
        }
    }
}
