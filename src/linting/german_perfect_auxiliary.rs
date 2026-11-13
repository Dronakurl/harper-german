//! *Sie hat gegangen*: the perfect of a verb of motion or change of state
//! takes *sein*.

use harper_core::{
    Document, Token, TokenKind, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
};

use crate::grammar::noun_phrase::lowercase_of;

/// Past participles whose perfect is formed with *sein* and never with
/// *haben*: *ich bin gegangen*, *er ist gestorben*, *es ist passiert*.
///
/// Only verbs with no transitive use that would take *haben*. *gefahren*,
/// *geflogen* and *geritten* are left out (*Er hat das Auto gefahren*), as is
/// *gefallen* (*das hat mir gefallen*) and every reflexive (*sich umgezogen*).
const SEIN_PARTICIPLES: &[&str] = &[
    "gegangen",
    "gekommen",
    "gestorben",
    "geblieben",
    "gewesen",
    "geworden",
    "passiert",
    "geschehen",
    "gelungen",
    "gereist",
    "gewachsen",
    "gestiegen",
    "gesunken",
    "geflohen",
    "geschlüpft",
    "aufgestanden",
    "eingeschlafen",
    "aufgewacht",
    "angekommen",
    "abgereist",
    "ausgestiegen",
    "eingestiegen",
    "umgestiegen",
    "losgegangen",
    "weggegangen",
    "zurückgekommen",
    "mitgekommen",
    "hingegangen",
    "hingefallen",
    "umgefallen",
    "verschwunden",
    "erschienen",
    "entstanden",
    "aufgetaucht",
    "zusammengebrochen",
];

/// Verbs of motion that take *sein* for a movement somewhere and *haben* with an object (*Er
/// hat das Auto gefahren*). Read with *sein* only when the clause names a destination with
/// *nach* and has no object.
const MOTION_PARTICIPLES: &[&str] = &[
    "gelaufen",
    "geflogen",
    "gefahren",
    "gerannt",
    "geschwommen",
    "gesprungen",
    "geritten",
    "geklettert",
    "gewandert",
    "gesegelt",
    "geradelt",
];

/// The accusative pronouns: an object or a reflexive in the clause makes *sein* with a
/// participle impossible (*Sie ist mich angerufen*, *Ich bin mich gefreut*). *uns* and *euch*
/// are datives too (*Er ist uns gefolgt*).
const ACCUSATIVE_PRONOUNS: &[&str] = &["mich", "dich", "ihn"];

/// Past participles whose perfect is formed with *haben* and never with
/// *sein*, and that have no state passive either: *ich habe gearbeitet*, *es
/// hat gedauert*. A transitive verb is left out, because *sein* with its
/// participle is the state passive (*das Buch ist gelesen*), and so are
/// *gewohnt* (*ich bin es gewohnt*) and the verbs of motion that take *sein*
/// as well (*getanzt*, *geschwommen*).
const HABEN_PARTICIPLES: &[&str] = &[
    "gearbeitet",
    "geschlafen",
    "gelacht",
    "geweint",
    "gewartet",
    "geredet",
    "telefoniert",
    "geschwiegen",
    "gezögert",
    "gefrühstückt",
    "übernachtet",
    "gedauert",
    "geklappt",
    "funktioniert",
    "gefehlt",
    "geholfen",
    "geantwortet",
    "gelebt",
];

/// Forms of *haben* and the *sein* form that replaces each.
const HABEN_TO_SEIN: &[(&str, &str)] = &[
    ("habe", "bin"),
    ("hast", "bist"),
    ("hat", "ist"),
    ("haben", "sind"),
    ("habt", "seid"),
    ("hatte", "war"),
    ("hattest", "warst"),
    ("hatten", "waren"),
    ("hattet", "wart"),
    ("hätte", "wäre"),
    ("hättest", "wärst"),
    ("hätten", "wären"),
    ("hättet", "wärt"),
];

/// Words that show the participle is not the main verb of a perfect with the
/// *haben* in front: a modal perfect (*hat gehen müssen*), a passive
/// (*hat ... gekommen werden*), a reflexive.
const BREAKERS: &[&str] = &[
    "werden", "worden", "müssen", "können", "dürfen", "sollen", "wollen", "lassen", "sich",
];

/// Forms of *sein*. A clause that already has one is a coordination or a
/// modal construction the simple pattern does not describe: *Sie haben
/// gesungen und sind eingeschlafen*, *Er wäre dafür zu haben gewesen*.
const SEIN_FORMS: &[&str] = &[
    "bin", "bist", "ist", "sind", "seid", "war", "warst", "waren", "wart", "wäre", "wärst",
    "wären", "wärt", "sei", "seien", "gewesen",
];

/// Requires *sein* in the perfect of a verb of motion or change of state:
/// *Sie hat gegangen* → *Sie ist gegangen*, *weil er gestorben hat* → *weil er
/// gestorben ist*.
///
/// The *haben* form and the participle have to be in the same clause, with the
/// participle at its end (main clause) or directly in front of the auxiliary
/// (subordinate clause). Only participles that never take *haben* are listed.
#[derive(Default)]
pub struct GermanPerfectAuxiliary;

impl GermanPerfectAuxiliary {
    fn sein_form(word: &str) -> Option<&'static str> {
        HABEN_TO_SEIN
            .iter()
            .find(|(haben, _)| *haben == word)
            .map(|(_, sein)| *sein)
    }

    fn report(auxiliary: &Token, sein: &str, participle: &str, document: &Document) -> Lint {
        let original = document.get_span_content(&auxiliary.span);
        Lint {
            span: auxiliary.span,
            lint_kind: LintKind::Grammar,
            suggestions: vec![Suggestion::replace_with_match_case(
                sein.chars().collect(),
                original,
            )],
            priority: 31,
            message: format!("»{participle}« bildet das Perfekt mit »sein«, nicht mit »haben«."),
        }
    }
}

impl GermanPerfectAuxiliary {
    fn haben_form(word: &str) -> Option<&'static str> {
        HABEN_TO_SEIN
            .iter()
            .find(|(_, sein)| *sein == word)
            .map(|(haben, _)| *haben)
    }

    /// The other direction: *ich bin gestern viel gearbeitet* → *habe*.
    fn sein_with_a_haben_participle(
        clause: &[&Token],
        lower: &[String],
        document: &Document,
    ) -> Option<Lint> {
        // A passive (*es ist gearbeitet worden*), a modal or a clause that
        // already has *haben* is not this pattern.
        if lower.iter().any(|word| {
            BREAKERS.contains(&word.as_str())
                || HABEN_TO_SEIN.iter().any(|(haben, _)| haben == word)
        }) {
            return None;
        }
        let last = lower.len().checked_sub(1)?;
        // *Sie ist mich angerufen*: with an accusative pronoun in the clause any participle
        // that is no verb of motion forms its perfect with *haben*.
        // The pronoun stands between the auxiliary and the participle, not behind a preposition
        // (*Für ihn ist … geplant*), and no other verb comes between them (*Ich bin … und ich
        // fühle mich aufgehoben*).
        let object_between = |auxiliary: usize, participle: usize| {
            let (from, to) = (auxiliary.min(participle), auxiliary.max(participle));
            (from + 1..to).any(|at| {
                ACCUSATIVE_PRONOUNS.contains(&lower[at].as_str())
                    && !clause[at - 1].kind.is_preposition()
            }) && !(from + 1..to).any(|at| {
                clause[at].kind.is_verb() && !ACCUSATIVE_PRONOUNS.contains(&lower[at].as_str())
            })
        };
        let object = lower
            .iter()
            .skip(1)
            .any(|word| ACCUSATIVE_PRONOUNS.contains(&word.as_str()));
        let is_participle = |at: usize| {
            HABEN_PARTICIPLES.contains(&lower[at].as_str())
                || (object
                    && !SEIN_PARTICIPLES.contains(&lower[at].as_str())
                    && !MOTION_PARTICIPLES.contains(&lower[at].as_str())
                    && lower[at].contains("ge")
                    && clause[at].kind.is_verb()
                    && matches!(&clause[at].kind, TokenKind::Word(Some(m)) if m
                        .verb
                        .as_ref()
                        .and_then(|v| v.verb_forms)
                        .is_some_and(|f| f.contains(harper_core::VerbFormFlags::PAST_PARTICIPLE))))
        };
        let (participle, auxiliary) = if is_participle(last) {
            // Main clause: auxiliary somewhere in front, participle last.
            let at = lower[..last]
                .iter()
                .position(|word| Self::haben_form(word).is_some())?;
            (last, at)
        } else if last >= 1 && is_participle(last - 1) && Self::haben_form(&lower[last]).is_some() {
            (last - 1, last)
        } else {
            return None;
        };
        if !HABEN_PARTICIPLES.contains(&lower[participle].as_str())
            && !object_between(auxiliary, participle)
        {
            return None;
        }
        let haben = Self::haben_form(&lower[auxiliary])?;
        let original = document.get_span_content(&clause[auxiliary].span);
        Some(Lint {
            span: clause[auxiliary].span,
            lint_kind: LintKind::Grammar,
            suggestions: vec![Suggestion::replace_with_match_case(
                haben.chars().collect(),
                original,
            )],
            priority: 31,
            message: format!(
                "»{}« bildet das Perfekt mit »haben«, nicht mit »sein«.",
                lower[participle]
            ),
        })
    }
}

impl Linter for GermanPerfectAuxiliary {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();

        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();

            // Split into clauses at punctuation.
            let mut start = 0;
            while start < words.len() {
                let end = (start..words.len())
                    .find(|&at| matches!(words[at].kind, TokenKind::Punctuation(_)))
                    .unwrap_or(words.len());
                let clause = &words[start..end];
                start = end + 1;

                let lower: Vec<String> = clause
                    .iter()
                    .map(|token| lowercase_of(token, document))
                    .collect();
                lints.extend(Self::sein_with_a_haben_participle(clause, &lower, document));
                if lower.iter().any(|word| {
                    BREAKERS.contains(&word.as_str()) || SEIN_FORMS.contains(&word.as_str())
                }) {
                    continue;
                }
                // *passiert* is also transitive — *nachdem das Gesetz das
                // Unterhaus passiert hatte* — so it is only read with no noun
                // in the clause that could be its object.
                let has_noun = clause.iter().skip(1).any(|token| {
                    document
                        .get_span_content(&token.span)
                        .first()
                        .is_some_and(|c| c.is_uppercase())
                });
                // *Wir haben nach Hause gelaufen*, *Er hat nach Paris geflogen*: a destination
                // and no object.
                let destination = lower.iter().enumerate().any(|(at, word)| {
                    word == "nach"
                        && clause.get(at + 1).is_some_and(|next| {
                            document
                                .get_span_content(&next.span)
                                .first()
                                .is_some_and(|c| c.is_uppercase())
                        })
                }) && !lower.iter().any(|word| {
                    [
                        "den", "das", "die", "einen", "ein", "eine", "ihn", "es", "sie", "uns",
                        "euch", "mich", "dich",
                    ]
                    .contains(&word.as_str())
                });
                let participle_ok = |word: &str| {
                    (SEIN_PARTICIPLES.contains(&word) && !(word == "passiert" && has_noun))
                        || (destination && MOTION_PARTICIPLES.contains(&word))
                };
                let Some(last) = clause.len().checked_sub(1) else {
                    continue;
                };

                // Main clause: auxiliary somewhere, participle last.
                if participle_ok(&lower[last])
                    && let Some((at, sein)) = lower[..last]
                        .iter()
                        .enumerate()
                        .filter(|(at, _)| *at == 0 || lower[at - 1] != "zu")
                        .find_map(|(at, word)| Self::sein_form(word).map(|sein| (at, sein)))
                {
                    lints.push(Self::report(clause[at], sein, &lower[last], document));
                    continue;
                }

                // Subordinate clause: participle directly before a final auxiliary.
                if last >= 1
                    && participle_ok(&lower[last - 1])
                    && let Some(sein) = Self::sein_form(&lower[last])
                {
                    lints.push(Self::report(clause[last], sein, &lower[last - 1], document));
                }
            }
        }

        lints
    }

    fn description(&self) -> &str {
        "Verlangt »sein« im Perfekt von Verben der Bewegung und Zustandsänderung (»sie ist gegangen«)."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanPerfectAuxiliary;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn fixes(text: &str) -> Vec<String> {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanPerfectAuxiliary
            .lint(&document)
            .into_iter()
            .flat_map(|lint| lint.suggestions)
            .map(|suggestion| suggestion.to_string())
            .collect()
    }

    #[test]
    fn haben_with_a_sein_participle_is_reported() {
        assert_eq!(fixes("Sie hat gegangen."), ["Replace with: “ist”"]);
        assert_eq!(
            fixes("Ich habe gestern nach Hause gekommen."),
            ["Replace with: “bin”"]
        );
        assert_eq!(
            fixes("Wir haben lange in Berlin geblieben."),
            ["Replace with: “sind”"]
        );
        assert_eq!(fixes("Was hat passiert?"), ["Replace with: “ist”"]);
        assert_eq!(
            fixes("Wir haben nach Hause gelaufen."),
            ["Replace with: “sind”"]
        );
        assert_eq!(
            fixes("Er hat nach Paris geflogen."),
            ["Replace with: “ist”"]
        );
        assert!(fixes("Er hat das Auto nach Paris gefahren.").is_empty());
        assert!(fixes("Er hat die Kinder nach Hause gefahren.").is_empty());
        assert!(fixes("Wir sind nach Hause gelaufen.").is_empty());
        assert_eq!(
            fixes("Hast du gut eingeschlafen?"),
            ["Replace with: “Bist”"]
        );
    }

    #[test]
    fn sein_with_a_haben_participle_is_reported() {
        assert_eq!(
            fixes("Ich bin gestern viel gearbeitet."),
            ["Replace with: “habe”"]
        );
        assert_eq!(fixes("Es ist lange gedauert."), ["Replace with: “hat”"]);
        assert_eq!(fixes("Sie ist mich angerufen."), ["Replace with: “hat”"]);
        assert_eq!(fixes("Ich bin mich gefreut."), ["Replace with: “habe”"]);
        assert!(fixes("Er ist mich besuchen gekommen.").is_empty());
        assert!(fixes("Sie ist mit uns gefahren.").is_empty());
        assert_eq!(
            fixes("Weil er gut geschlafen ist, war er wach."),
            ["Replace with: “hat”"]
        );
        for text in [
            "Ich habe gestern viel gearbeitet.",
            "Es ist viel gearbeitet worden.",
            "Ich bin es gewohnt.",
            "Das Buch ist gelesen.",
            "Er ist gegangen und hat gelacht.",
        ] {
            assert!(fixes(text).is_empty(), "{text}: {:?}", fixes(text));
        }
    }

    #[test]
    fn a_subordinate_clause_is_checked_at_its_end() {
        assert_eq!(
            fixes("Ich weiß, dass er gestorben hat."),
            ["Replace with: “ist”"]
        );
        assert_eq!(
            fixes("Weil wir zu spät angekommen haben, war alles vorbei."),
            ["Replace with: “sind”"]
        );
    }

    #[test]
    fn the_correct_auxiliary_is_quiet() {
        for text in [
            "Sie ist gegangen.",
            "Ich bin nach Hause gekommen.",
            "Er hat das Auto gefahren.",
            "Das hat mir gefallen.",
            "Er hat gehen müssen.",
            "Sie hat sich umgezogen.",
            "Ich habe ihn kommen sehen.",
            "Ich habe keine Lust, nach Hause zu gehen.",
            "Er hat einen Brief geschrieben.",
            "Wir hatten Glück, dass niemand gekommen ist.",
            "Sie haben gesungen und sind eingeschlafen.",
            "Er wäre dafür zu haben gewesen.",
            "Nachdem das Gesetz das Unterhaus passiert hatte, trat es in Kraft.",
        ] {
            assert!(fixes(text).is_empty(), "{text}");
        }
    }
}
