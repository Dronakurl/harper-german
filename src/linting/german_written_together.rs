use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Span, TokenKind};

/// Articles that form one word with *selbe*: *dieselbe*, *denselben*, *desselben*.
const SELB_ARTICLES: &[&str] = &["der", "die", "das", "den", "dem", "des"];

/// The forms of *selb-* that follow them. *Im selben*, *am selben* and *zur selben* are
/// contractions and correct apart, so they are not articles here.
const SELB_FORMS: &[&str] = &["selbe", "selben"];

const WEEKDAYS: &[&str] = &[
    "Montag",
    "Dienstag",
    "Mittwoch",
    "Donnerstag",
    "Freitag",
    "Samstag",
    "Sonntag",
];

const TIMES_OF_DAY: &[&str] = &[
    "Morgen",
    "Vormittag",
    "Mittag",
    "Nachmittag",
    "Abend",
    "Nacht",
];

/// Prepositions that form one word with *einander*: *miteinander*, *voneinander*.
const EINANDER_PREPOSITIONS: &[&str] = &[
    "an", "auf", "aus", "bei", "durch", "für", "gegen", "hinter", "in", "mit", "nach", "neben",
    "über", "um", "unter", "von", "vor", "zu",
];

/// Words that make a colour lighter or darker and are written together with it: *hellblau*,
/// *dunkelrote*, *blassgrün*.
const SHADES: &[&str] = &["hell", "dunkel", "blass", "tief", "knall"];

/// Colours, as the stem the declined forms add their ending to.
const COLOURS: &[&str] = &[
    "blau", "rot", "grün", "gelb", "grau", "braun", "lila", "rosa", "orange", "violett", "türkis",
    "beige", "weiß", "schwarz", "pink", "bunt",
];

/// Is `word` a colour or one of its declined forms (*blaue*, *roten*)?
fn is_colour(word: &str) -> bool {
    COLOURS.iter().any(|colour| {
        word.strip_prefix(colour)
            .is_some_and(|ending| ["", "e", "en", "er", "es", "em"].contains(&ending))
    })
}

/// Adjectives a writer splits into two words that are each a word: *Not wendige* is
/// *notwendige*.
const SPLIT_ADJECTIVES: &[(&str, &str)] = &[
    ("not", "wendig"),
    ("not", "dürftig"),
    ("nächst", "möglich"),
    ("schnellst", "möglich"),
    ("best", "möglich"),
    ("größt", "möglich"),
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
///   *wievielte* is one word and stays;
/// * *statt dessen* before anything but a noun, and a weekday with a time of day: *Dienstag
///   Abend* → *Dienstagabend*;
/// * a shade and a colour: *hell blau* → *hellblau*, *ein dunkel rotes Kleid* →
///   *dunkelrotes*; and *Not wendige* → *notwendige*.
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
            // *Dienstag Abend* → *Dienstagabend*: a weekday and a time of day are one word.
            let second_text: String = second.iter().collect();
            if WEEKDAYS.contains(&first.iter().collect::<String>().as_str())
                && TIMES_OF_DAY.contains(&second_text.as_str())
            {
                let joined: String = first
                    .iter()
                    .copied()
                    .chain(second_text.to_lowercase().chars())
                    .collect();
                lints.push(Lint {
                    span: Span::new(token.span.start, next.span.end),
                    lint_kind: LintKind::Spelling,
                    suggestions: vec![Suggestion::ReplaceWith(joined.chars().collect())],
                    message: format!(
                        "Wochentag und Tageszeit werden zusammengeschrieben: »{joined}«."
                    ),
                    priority: 31,
                });
                continue;
            }
            // The second half in lower case only: *Die Selben* may be a name.
            if second.first().is_some_and(|c| c.is_uppercase()) {
                continue;
            }
            let second_lower = lowercase(second);

            // *hell blau* → *hellblau*, *Not wendige* → *notwendige*. The first word keeps
            // its case only at the start of a sentence; *Not* is a noun on its own.
            let shade = SHADES.contains(&first_lower.as_str()) && is_colour(&second_lower);
            let split_adjective = SPLIT_ADJECTIVES.iter().any(|(head, tail)| {
                first_lower == *head
                    && second_lower
                        .strip_prefix(tail)
                        .is_some_and(|ending| ["", "e", "en", "er", "es", "em"].contains(&ending))
            });
            if shade || split_adjective {
                let sentence_start = (0..at)
                    .rev()
                    .find(|&i| !tokens[i].kind.is_whitespace())
                    .is_none_or(|i| tokens[i].kind.is_sentence_terminator());
                let joined_text = format!("{first_lower}{second_lower}");
                let joined: Vec<char> = if sentence_start {
                    in_case_of(&joined_text, first)
                } else {
                    joined_text.chars().collect()
                };
                lints.push(Lint {
                    span: Span::new(token.span.start, next.span.end),
                    lint_kind: LintKind::Spelling,
                    suggestions: vec![Suggestion::ReplaceWith(joined.clone())],
                    message: format!(
                        "»{}« wird zusammengeschrieben.",
                        joined.iter().collect::<String>()
                    ),
                    priority: 31,
                });
                continue;
            }

            // *Statt dessen gingen wir* → *Stattdessen*; *statt dessen Bruder* is a genitive.
            let stattdessen = first_lower == "statt"
                && second_lower == "dessen"
                && !tokens.get(at + 4).is_some_and(|t| {
                    t.kind.is_word()
                        && document
                            .get_span_content(&t.span)
                            .first()
                            .is_some_and(|c| c.is_uppercase())
                });
            let joins = stattdessen
                || (SELB_ARTICLES.contains(&first_lower.as_str())
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
    fn joins_a_shade_and_a_colour() {
        assert_eq!(fixes("Das Auto ist hell blau."), ["hellblau"]);
        assert_eq!(fixes("Sie trägt ein dunkel rotes Kleid."), ["dunkelrotes"]);
        assert_eq!(fixes("Hell grüne Wände gefallen mir."), ["Hellgrüne"]);
        assert_eq!(fixes("Das ist eine Not wendige Maßnahme."), ["notwendige"]);
        assert_eq!(
            fixes("Ich kündige zum nächst möglichen Zeitpunkt."),
            ["nächstmöglichen"]
        );
        assert_eq!(fixes("Der Himmel war tief schwarz."), ["tiefschwarz"]);
        assert!(fixes("Es wurde hell, blau leuchtete der Himmel.").is_empty());
        assert!(fixes("Das Zimmer ist hell und blau.").is_empty());
        assert!(fixes("Das Licht ist hell genug.").is_empty());
        assert!(fixes("Er geriet in Not wegen der Schulden.").is_empty());
        assert!(fixes("Das Auto ist hellblau.").is_empty());
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
        assert_eq!(fixes("Statt dessen gingen wir ins Kino."), ["Stattdessen"]);
        assert_eq!(fixes("Wir sehen uns Dienstag Abend."), ["Dienstagabend"]);
        assert_eq!(fixes("Bis Montag Morgen!"), ["Montagmorgen"]);
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
            "Er kam statt dessen Bruder.",
            "Sie kommt Dienstag, Abend ist schlecht.",
            "Wir treffen uns am Dienstag abends.",
        ] {
            assert!(fixes(text).is_empty(), "{text}: {:?}", fixes(text));
        }
    }
}
