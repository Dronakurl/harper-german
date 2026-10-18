//! The case that a verb with a fixed preposition asks for.

use crate::grammar::noun_gender::NounGender;
use crate::morphology::Gender;
use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Token, TokenKind, TokenStringExt};

#[derive(Clone, Copy, PartialEq)]
enum Case {
    Accusative,
    Dative,
}

/// A verb with the preposition it governs and the case behind it. The forms are written out:
/// a stem and endings would also catch other verbs (*denken* and *danken* share an ending, and
/// *gedacht* is no *denk-* form).
struct Governed {
    forms: &'static [&'static str],
    preposition: &'static [&'static str],
    case: Case,
}

const VERBS: &[Governed] = &[
    Governed {
        forms: &[
            "denke", "denkst", "denkt", "denken", "dachte", "dachtest", "dachten", "gedacht",
        ],
        preposition: &["an"],
        case: Case::Accusative,
    },
    Governed {
        forms: &[
            "glaube", "glaubst", "glaubt", "glauben", "glaubte", "glaubten", "geglaubt",
        ],
        preposition: &["an"],
        case: Case::Accusative,
    },
    Governed {
        forms: &[
            "freue", "freust", "freut", "freuen", "freute", "freuten", "gefreut",
        ],
        preposition: &["auf", "über"],
        case: Case::Accusative,
    },
    Governed {
        forms: &[
            "spreche",
            "sprichst",
            "spricht",
            "sprechen",
            "sprach",
            "sprachen",
            "gesprochen",
        ],
        preposition: &["über"],
        case: Case::Accusative,
    },
    Governed {
        forms: &[
            "rede", "redest", "redet", "reden", "redete", "redeten", "geredet",
        ],
        preposition: &["über"],
        case: Case::Accusative,
    },
    Governed {
        forms: &[
            "diskutiere",
            "diskutierst",
            "diskutiert",
            "diskutieren",
            "diskutierte",
            "diskutierten",
        ],
        preposition: &["über"],
        case: Case::Accusative,
    },
    Governed {
        forms: &[
            "arbeite",
            "arbeitest",
            "arbeitet",
            "arbeiten",
            "arbeitete",
            "arbeiteten",
            "gearbeitet",
        ],
        preposition: &["an"],
        case: Case::Dative,
    },
    Governed {
        forms: &[
            "zweifle",
            "zweifelst",
            "zweifelt",
            "zweifeln",
            "zweifelte",
            "zweifelten",
            "gezweifelt",
        ],
        preposition: &["an"],
        case: Case::Dative,
    },
];

const REFLEXIVE: &[&str] = &["mich", "dich", "sich", "uns", "euch"];

/// A noun after *an dem* that names a time or a place: *Ich dachte an dem Abend noch lange
/// darüber nach* has the dative of a time, and the verb is not what governs *an*.
const TIME_AND_PLACE: &[&str] = &[
    "Tag",
    "Abend",
    "Morgen",
    "Nachmittag",
    "Mittag",
    "Wochenende",
    "Zeitpunkt",
    "Ort",
    "Platz",
    "Stelle",
    "Moment",
    "Datum",
    "Termin",
    "Bahnhof",
    "Tisch",
];

/// Dative singular forms where the verb asks for the accusative, and the accusative of each for
/// the masculine and the neuter: *dem*, *einem*, ... only singular, since *meinen* is the
/// masculine accusative and the dative plural at once.
const DATIVE_TO_ACCUSATIVE: &[(&str, &str, &str)] = &[
    ("dem", "den", "das"),
    ("einem", "einen", "ein"),
    ("diesem", "diesen", "dieses"),
    ("meinem", "meinen", "mein"),
    ("deinem", "deinen", "dein"),
    ("seinem", "seinen", "sein"),
    ("unserem", "unseren", "unser"),
    ("keinem", "keinen", "kein"),
];

/// Accusative forms only the neuter has (and the masculine does not share with the dative plural).
const NEUTER_ACCUSATIVE: &[&str] = &[
    "das", "ein", "dieses", "mein", "dein", "sein", "kein", "unser",
];

/// *Er denkt an dem Urlaub* wants *an den Urlaub*; *Wir arbeiten an das Projekt* wants *an dem*.
///
/// A short, deliberate list of verbs whose preposition is part of the verb (*denken an*, *sprechen
/// über*). The preposition has to stand right behind the verb (or its reflexive pronoun) and an
/// article with an unambiguous singular case right behind that, so *an dem Tag* as a time
/// adjunct, and everything with a plural, is out of reach by construction.
pub struct GermanVerbPreposition {
    nouns: NounGender,
}

impl Default for GermanVerbPreposition {
    fn default() -> Self {
        Self::new()
    }
}

impl GermanVerbPreposition {
    pub fn new() -> Self {
        Self {
            nouns: NounGender::new(),
        }
    }
}

impl Linter for GermanVerbPreposition {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let tokens: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            let text = |token: &Token| -> String {
                document.get_span_content(&token.span).iter().collect()
            };
            let is_word = |token: &Token| matches!(token.kind, TokenKind::Word(_));

            for (index, token) in tokens.iter().enumerate() {
                if !is_word(token) {
                    continue;
                }
                let verb = text(token);
                // A lower-case form: *Denken an dem …* with a capital is the noun.
                if !verb.chars().next().is_some_and(char::is_lowercase) {
                    continue;
                }
                let Some(governed) = VERBS.iter().find(|g| g.forms.contains(&verb.as_str())) else {
                    continue;
                };
                let mut at = index + 1;
                if tokens.get(at).is_some_and(|t| {
                    is_word(t) && REFLEXIVE.contains(&text(t).to_lowercase().as_str())
                }) {
                    at += 1;
                }
                let (Some(preposition), Some(article)) = (tokens.get(at), tokens.get(at + 1))
                else {
                    continue;
                };
                if !is_word(preposition)
                    || !is_word(article)
                    || !governed
                        .preposition
                        .contains(&text(preposition).to_lowercase().as_str())
                {
                    continue;
                }
                let article_text = text(article).to_lowercase();
                let noun = tokens.get(at + 2).filter(|t| is_word(t)).map(|t| text(t));
                if noun.as_deref().is_some_and(|n| TIME_AND_PLACE.contains(&n)) {
                    continue;
                }

                match governed.case {
                    Case::Accusative => {
                        let Some(&(_, masculine, neuter)) = DATIVE_TO_ACCUSATIVE
                            .iter()
                            .find(|(dative, _, _)| *dative == article_text)
                        else {
                            continue;
                        };
                        let gender = noun
                            .as_deref()
                            .and_then(|n| self.nouns.gender_of(n))
                            .map(|(gender, _)| gender);
                        let fixed = match gender {
                            Some(Gender::Masculine) => Some(masculine),
                            Some(Gender::Neuter) => Some(neuter),
                            _ => None,
                        };
                        lints.push(Lint {
                            span: article.span,
                            lint_kind: LintKind::Grammar,
                            suggestions: fixed
                                .map(|f| Suggestion::ReplaceWith(f.chars().collect()))
                                .into_iter()
                                .collect(),
                            priority: 30,
                            message: format!(
                                "»{verb} {}« verlangt den Akkusativ, nicht den Dativ.",
                                text(preposition)
                            ),
                        });
                    }
                    Case::Dative => {
                        if !NEUTER_ACCUSATIVE.contains(&article_text.as_str()) {
                            continue;
                        }
                        // Only for a neuter noun that is surely one: *das* is also the relative
                        // pronoun and a demonstrative.
                        let neuter = noun
                            .as_deref()
                            .and_then(|n| self.nouns.gender_of(n))
                            .is_some_and(|(gender, surely)| gender == Gender::Neuter && surely);
                        if !neuter {
                            continue;
                        }
                        let dative = match article_text.as_str() {
                            "das" => "dem",
                            "ein" => "einem",
                            "dieses" => "diesem",
                            "mein" => "meinem",
                            "dein" => "deinem",
                            "sein" => "seinem",
                            "kein" => "keinem",
                            _ => "unserem",
                        };
                        lints.push(Lint {
                            span: article.span,
                            lint_kind: LintKind::Grammar,
                            suggestions: vec![Suggestion::ReplaceWith(dative.chars().collect())],
                            priority: 30,
                            message: format!(
                                "»{verb} {}« verlangt den Dativ, nicht den Akkusativ.",
                                text(preposition)
                            ),
                        });
                    }
                }
            }
        }
        lints
    }

    fn description(&self) -> &str {
        "Prüft den Fall hinter Verben mit fester Präposition (»denken an«, »sprechen über«)."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanVerbPreposition;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn count(text: &str) -> usize {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanVerbPreposition::new().lint(&document).len()
    }

    #[test]
    fn the_case_behind_the_preposition_is_the_verbs() {
        for text in [
            "Er denkt an dem Urlaub.",
            "Ich glaube an einem Gott.",
            "Wir freuen uns auf dem Wochenendausflug.",
            "Sie sprechen über dem Thema.",
            "Wir arbeiten an das Projekt.",
            "Er zweifelt an das Ergebnis.",
        ] {
            assert_eq!(count(text), 1, "{text}");
        }
    }

    #[test]
    fn the_right_case_and_the_adjuncts_are_quiet() {
        for text in [
            "Er denkt an den Urlaub.",
            "Ich glaube an einen Gott.",
            "Ich dachte an dem Abend noch lange darüber nach.",
            "Wir arbeiten an dem Projekt.",
            "Das Denken an dem Ort ist schwer.",
            "Er denkt an dem Tisch nach.",
            "Wir sprechen über den Plan.",
            "Er arbeitet an das, was kommt.",
        ] {
            assert_eq!(count(text), 0, "{text}");
        }
    }
}
