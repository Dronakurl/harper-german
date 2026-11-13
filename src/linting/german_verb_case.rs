//! The case a verb asks for: *ich erinnere mich an **dir***, *er hilft
//! **seinen** Freund*, *die Brille liegt auf **den** Tisch*.

use std::sync::Arc;

use harper_core::{
    Document, Punctuation, Token, TokenKind, TokenStringExt,
    linting::{Lint, LintKind, Linter, Suggestion},
    spell::Dictionary,
};

use crate::grammar::determiners::{
    DeterminerReading, determiner_readings, forms_for_readings, readings_allowed_by,
    readings_allowed_by_spelling,
};
use crate::grammar::noun_phrase::lowercase_of;
use crate::grammar::verbs::joined_separable_verb;
use crate::morphology::{Case, CaseSet, MorphologyExt};
use crate::spell::compact_dict::CompactDictionary;
use crate::spell::curated_german_dictionary;

/// Verbs whose prepositional object is in the accusative although the
/// preposition also takes the dative: *denken an*, *warten auf*, *sich freuen
/// über*. The flag says whether a determiner is checked as well as a
/// pronoun.
///
/// A two-way preposition with the dative is a place, and these verbs can have
/// one: *ich warte auf dem Bahnsteig*, *er freute sich auf dem Balkon*. A
/// person pronoun is never a place, so *auf dir* and *an ihm* are always the
/// object. Only *über* and *an* — where *über dem Sofa* and *an dem Tisch*
/// are rare beside these verbs — check a determiner too, and *an* not before a
/// word of time (*an dem Tag dachte er …*).
const ACCUSATIVE_OBJECTS: &[(&str, &[&str], bool)] = &[
    (
        "an",
        &["denken", "erinnern", "glauben", "gewöhnen", "zurückdenken"],
        true,
    ),
    (
        "auf",
        &[
            "warten",
            "hoffen",
            "achten",
            "aufpassen",
            "verlassen",
            "konzentrieren",
            "freuen",
            "verzichten",
            "reagieren",
        ],
        false,
    ),
    (
        "über",
        &[
            "freuen",
            "ärgern",
            "sprechen",
            "reden",
            "nachdenken",
            "beschweren",
            "diskutieren",
            "lachen",
            "wundern",
            "aufregen",
            "informieren",
            "klagen",
            "streiten",
        ],
        true,
    ),
];

/// Verbs whose prepositional object is in the dative: *teilnehmen an*,
/// *leiden an*, *sich fürchten vor*. Here the accusative is the mistake, and a
/// place reading does not exist for these verbs.
const DATIVE_OBJECTS: &[(&str, &[&str])] = &[
    (
        "an",
        &[
            "teilnehmen",
            "leiden",
            "zweifeln",
            "arbeiten",
            "sterben",
            "erkranken",
            "mangeln",
        ],
    ),
    (
        "vor",
        &[
            "fürchten",
            "warnen",
            "schützen",
            "retten",
            "fliehen",
            "ekeln",
            "verstecken",
        ],
    ),
];

/// Verbs of position: what they place somewhere is in the dative, *die Brille
/// liegt auf dem Tisch*. Their counterparts of motion (*legen*, *setzen*,
/// *stellen*) take the accusative and are not here. *stehen* is left out, for
/// *ich stehe auf den Typen*.
const POSITION_VERBS: &[&str] = &[
    "liegen", "sitzen", "wohnen", "leben", "schlafen", "befinden",
];

const TWO_WAY_PREPOSITIONS: &[&str] = &[
    "an", "auf", "in", "über", "unter", "vor", "hinter", "neben", "zwischen",
];

/// Verbs with a dative object and no accusative one: *helfen*, *danken*,
/// *folgen*.
const DATIVE_VERBS: &[&str] = &[
    "helfen",
    "danken",
    "folgen",
    "gratulieren",
    "vertrauen",
    "begegnen",
    "gehören",
    "gefallen",
    "zuhören",
    "widersprechen",
    "schaden",
    "ähneln",
    "gehorchen",
    "nützen",
    "zustimmen",
    "glauben",
];

/// Verbs that take an accusative object with an infinitive of its own: in *lass
/// mich helfen* the *mich* belongs to *lassen*.
const ACCUSATIVE_WITH_INFINITIVE: &[&str] = &["lassen", "sehen", "hören", "fühlen", "spüren"];

/// The strong and irregular forms of the verbs above. The dictionary has
/// them as entries of their own, with no link to the infinitive the regular
/// forms carry.
const STRONG_FORMS: &[(&str, &[&str])] = &[
    (
        "helfen",
        &[
            "hilf", "hilfst", "hilft", "half", "halfen", "halfst", "geholfen",
        ],
    ),
    ("lassen", &["lass", "lässt", "ließ", "ließen", "gelassen"]),
    (
        "sehen",
        &["sieh", "siehst", "sieht", "sah", "sahen", "gesehen"],
    ),
    ("gehören", &["gehört", "gehörte", "gehörten"]),
    ("denken", &["dachte", "dachten", "dachtest", "gedacht"]),
    ("nachdenken", &["nachgedacht"]),
    (
        "sprechen",
        &[
            "sprich",
            "sprichst",
            "spricht",
            "sprach",
            "sprachen",
            "gesprochen",
        ],
    ),
    ("teilnehmen", &["teilgenommen", "teilzunehmen"]),
    (
        "nehmen",
        &["nimm", "nimmst", "nimmt", "nahm", "nahmen", "genommen"],
    ),
    ("leiden", &["litt", "litten", "gelitten"]),
    ("sterben", &["stirbt", "starb", "starben", "gestorben"]),
    ("gefallen", &["gefällt", "gefiel", "gefielen"]),
    (
        "schlafen",
        &["schläft", "schlief", "schliefen", "geschlafen"],
    ),
    ("liegen", &["lag", "lagen", "gelegen"]),
    ("sitzen", &["saß", "saßen", "gesessen"]),
    ("befinden", &["befand", "befanden"]),
    ("fliehen", &["floh", "flohen", "geflohen"]),
    (
        "widersprechen",
        &["widerspricht", "widersprach", "widersprochen"],
    ),
    ("verlassen", &["verlässt", "verließ", "verließen"]),
    ("streiten", &["stritt", "stritten", "gestritten"]),
    ("schützen", &["geschützt"]),
    (
        "werfen",
        &["wirf", "wirfst", "wirft", "warf", "warfen", "geworfen"],
    ),
    ("vertrauen", &["vertraut"]),
];

/// Words of time, after which *an* is a date and not an object: *an dem Tag*.
const TIME_WORDS: &[&str] = &[
    "Tag",
    "Abend",
    "Morgen",
    "Mittag",
    "Nachmittag",
    "Vormittag",
    "Wochenende",
    "Montag",
    "Dienstag",
    "Mittwoch",
    "Donnerstag",
    "Freitag",
    "Samstag",
    "Sonntag",
    "Feiertag",
    "Punkt",
    "Ende",
    "Anfang",
    "Datum",
    "Termin",
];

/// The pronouns that show their case, with the form of the other case. *jemanden* and
/// *niemanden* are accusative only, so they are never the subject.
const DATIVE_TO_ACCUSATIVE: &[(&str, &str)] = &[
    ("mir", "mich"),
    ("dir", "dich"),
    ("ihm", "ihn"),
    ("jemandem", "jemanden"),
    ("niemandem", "niemanden"),
    ("Jemandem", "Jemanden"),
    ("Niemandem", "Niemanden"),
];

pub struct GermanVerbCase {
    dictionary: Arc<CompactDictionary>,
}

impl Default for GermanVerbCase {
    fn default() -> Self {
        Self {
            dictionary: curated_german_dictionary(),
        }
    }
}

/// One clause: the words between two commas, colons or semicolons, and the
/// conjunctions that start a new one.
fn clauses<'a>(words: &[&'a Token], document: &Document) -> Vec<Vec<&'a Token>> {
    let mut clauses = vec![Vec::new()];
    for &token in words {
        let breaks = match &token.kind {
            TokenKind::Punctuation(_) => {
                matches!(
                    document.get_span_content(&token.span),
                    [',' | ';' | ':' | '–' | '(' | ')']
                )
            }
            TokenKind::Word(_) => matches!(
                lowercase_of(token, document).as_str(),
                "und" | "oder" | "aber" | "sondern" | "denn" | "dass" | "weil" | "wenn" | "als"
            ),
            _ => false,
        };
        if breaks {
            clauses.push(Vec::new());
        } else {
            clauses.last_mut().unwrap().push(token);
        }
    }
    clauses
}

impl GermanVerbCase {
    /// The infinitives `word` may be a form of, including itself.
    fn lemmas(&self, word: &str) -> Vec<String> {
        let chars: Vec<char> = word.chars().collect();
        let mut lemmas = vec![word.to_owned()];
        lemmas.extend(
            STRONG_FORMS
                .iter()
                .filter(|(_, forms)| forms.contains(&word))
                .map(|(lemma, _)| (*lemma).to_owned()),
        );
        if lemmas.len() > 1 {
            return lemmas;
        }
        if let Some(metadata) = self.dictionary.get_word_metadata(&chars) {
            if metadata.verb.is_none() {
                return Vec::new();
            }
            lemmas.extend(
                metadata
                    .derived_from
                    .iter()
                    .filter_map(|id| self.dictionary.get_word_from_id(id))
                    .map(|lemma| lemma.iter().collect::<String>().to_lowercase()),
            );
        }
        lemmas
    }

    /// Every verb the clause contains, by infinitive, with the separable
    /// particle at its end joined on: *nehme … teil* is *teilnehmen*.
    fn verbs_of(&self, clause: &[&Token], document: &Document) -> Vec<String> {
        // A capital in mid-clause is a noun: *Folge*, *Leben*, *Arbeiten*.
        let words: Vec<String> = clause
            .iter()
            .enumerate()
            .filter(|(at, t)| {
                t.kind.is_word()
                    && (*at == 0
                        || document
                            .get_span_content(&t.span)
                            .first()
                            .is_some_and(|c| c.is_lowercase()))
            })
            .map(|(_, t)| lowercase_of(t, document))
            .collect();
        let mut verbs: Vec<String> = words.iter().flat_map(|w| self.lemmas(w)).collect();
        if let Some(particle) = words.last() {
            for word in &words[..words.len().saturating_sub(1)] {
                if let Some(joined) = joined_separable_verb(particle, word) {
                    verbs.extend(self.lemmas(&joined));
                }
            }
        }
        verbs
    }

    /// The determiner readings after narrowing by the noun at `noun`, if it is
    /// a capitalized word right behind the determiner.
    fn narrowed(
        &self,
        determiner: &str,
        noun: Option<&&Token>,
        document: &Document,
    ) -> Option<Vec<DeterminerReading>> {
        let readings = determiner_readings(determiner)?;
        let noun = noun.filter(|t| t.kind.is_word())?;
        let head = document.get_span_content_str(&noun.span);
        if !head.chars().next().is_some_and(char::is_uppercase) {
            return None;
        }
        let chars: Vec<char> = head.chars().collect();
        let agreement = self
            .dictionary
            .get_word_metadata(&chars)
            .filter(|m| m.is_noun())
            .map(|m| m.noun_agreement())
            .unwrap_or_default();
        Some(readings_allowed_by(
            &readings_allowed_by_spelling(readings, &head),
            &agreement,
        ))
    }

    fn report(token: &Token, forms: Vec<String>, message: String) -> Option<Lint> {
        if forms.is_empty() {
            return None;
        }
        Some(Lint {
            span: token.span,
            lint_kind: LintKind::Grammar,
            suggestions: forms
                .iter()
                .map(|form| Suggestion::ReplaceWith(form.chars().collect()))
                .collect(),
            message,
            priority: 31,
        })
    }

    /// The object of a preposition after a verb that fixes its case.
    fn prepositional_object(
        &self,
        clause: &[&Token],
        verbs: &[String],
        document: &Document,
    ) -> Vec<Lint> {
        let mut lints = Vec::new();
        for (at, token) in clause.iter().enumerate() {
            let preposition = lowercase_of(token, document);
            let Some(object) = clause.get(at + 1).filter(|t| t.kind.is_word()) else {
                continue;
            };
            let written = document.get_span_content_str(&object.span);
            let after = clause.get(at + 2);

            for (wanted, verbs_wanting) in [
                (Case::Accusative, accusative_verbs(&preposition)),
                (Case::Dative, dative_verbs(&preposition)),
            ] {
                let Some((list, determiners_too)) = verbs_wanting else {
                    continue;
                };
                let Some(verb) = verbs.iter().find(|v| list.contains(&v.as_str())) else {
                    continue;
                };
                let lint = if let Some(&(dative, accusative)) = DATIVE_TO_ACCUSATIVE
                    .iter()
                    .find(|(d, a)| written == *d || written == *a)
                {
                    let (bad, good) = match wanted {
                        Case::Accusative => (dative, accusative),
                        _ => (accusative, dative),
                    };
                    (written == bad)
                        .then(|| {
                            Self::report(
                                object,
                                vec![good.to_owned()],
                                format!(
                                    "»{verb} {preposition}« verlangt hier »{good}«, nicht »{bad}«."
                                ),
                            )
                        })
                        .flatten()
                } else if determiners_too
                    && !(preposition == "an"
                        && after.is_some_and(|t| {
                            TIME_WORDS.contains(&document.get_span_content_str(&t.span).as_str())
                        }))
                {
                    self.wrong_determiner(object, after, wanted, document, || {
                        format!("»{verb} {preposition}« verlangt den {}.", case_name(wanted))
                    })
                } else {
                    None
                };
                lints.extend(lint);
            }
        }
        lints
    }

    /// The determiner at `object` in the wrong case, if the noun after it
    /// leaves no reading in `wanted`.
    fn wrong_determiner(
        &self,
        object: &Token,
        noun: Option<&&Token>,
        wanted: Case,
        document: &Document,
        message: impl FnOnce() -> String,
    ) -> Option<Lint> {
        let written = document.get_span_content_str(&object.span);
        if written.chars().next().is_some_and(char::is_uppercase) {
            return None;
        }
        let readings = self.narrowed(&written, noun, document)?;
        if readings.is_empty() || readings.iter().any(|r| r.case == wanted) {
            return None;
        }
        let forms: Vec<String> = forms_for_readings(&readings, CaseSet::from(wanted))
            .into_iter()
            .map(str::to_owned)
            .collect();
        Self::report(object, forms, message())
    }

    /// The dative object of *helfen*, *danken*, *folgen* written in the
    /// accusative: *er hat mich geholfen*, *er hilft seinen Freund*.
    fn dative_object(&self, clause: &[&Token], verbs: &[String], document: &Document) -> Vec<Lint> {
        let Some(verb) = verbs.iter().find(|v| DATIVE_VERBS.contains(&v.as_str())) else {
            return Vec::new();
        };
        if verbs
            .iter()
            .any(|v| ACCUSATIVE_WITH_INFINITIVE.contains(&v.as_str()))
        {
            return Vec::new();
        }
        // *Ich glaube ihn zu kennen*: the accusative belongs to the infinitive.
        if verb == "glauben" && clause.iter().any(|t| lowercase_of(t, document) == "zu") {
            return Vec::new();
        }
        let is_dative_verb =
            |t: &Token| t.kind.is_word() && self.lemmas(&lowercase_of(t, document)).contains(verb);
        let mut lints = Vec::new();
        for (at, token) in clause.iter().enumerate() {
            let written = document.get_span_content_str(&token.span);
            // *…, jemanden zu schaden*: an infinitive group that opens with the
            // pronoun. Only at the start of the clause: in *er bat mich zu helfen*
            // the *mich* belongs to *bitten*.
            let before_zu_infinitive = at == 0
                && clause
                    .get(at + 1)
                    .is_some_and(|t| lowercase_of(t, document) == "zu")
                && clause.get(at + 2).is_some_and(|t| is_dative_verb(t));
            let next_to_verb = at.checked_sub(1).is_some_and(|i| is_dative_verb(clause[i]))
                || clause.get(at + 1).is_some_and(|t| is_dative_verb(t))
                || before_zu_infinitive;
            if !next_to_verb {
                continue;
            }
            // *Ich danke Sie*: the polite *Sie* behind the verb is the object
            // when a subject stands in front of it; *Danken Sie ihm* has it as
            // the subject.
            if written == "Sie"
                && at.checked_sub(1).is_some_and(|i| is_dative_verb(clause[i]))
                && at.checked_sub(2).is_some_and(|i| {
                    ["ich", "wir", "er", "sie", "man"]
                        .contains(&lowercase_of(clause[i], document).as_str())
                })
            {
                lints.extend(Self::report(
                    token,
                    vec!["Ihnen".to_owned()],
                    format!("»{verb}« verlangt den Dativ: »Ihnen«, nicht »Sie«."),
                ));
                continue;
            }
            if let Some(&(dative, accusative)) =
                DATIVE_TO_ACCUSATIVE.iter().find(|(_, a)| written == *a)
            {
                lints.extend(Self::report(
                    token,
                    vec![dative.to_owned()],
                    format!("»{verb}« verlangt den Dativ: »{dative}«, nicht »{accusative}«."),
                ));
            }
            // A determiner is not checked: after the verb the subject may stand
            // too, *dazu gehören das Sieb und …*, *es folgt eine Übersicht*.
        }
        lints
    }

    /// The accusative object of *fragen* written in the dative: *er fragt mir
    /// nach dem Weg* → *mich*. Only the pronouns that show the case, and only
    /// next to the verb.
    fn accusative_object(
        &self,
        clause: &[&Token],
        verbs: &[String],
        document: &Document,
    ) -> Vec<Lint> {
        const ACCUSATIVE_VERBS: &[&str] = &[
            "fragen",
            "besuchen",
            "anrufen",
            "unterstützen",
            "kennen",
            "lieben",
            "vermissen",
            "treffen",
            "verstehen",
            "brauchen",
            "einladen",
        ];
        let Some(verb) = verbs
            .iter()
            .find(|v| ACCUSATIVE_VERBS.contains(&v.as_str()))
        else {
            return Vec::new();
        };
        let is_verb = |t: &Token| {
            t.kind.is_word()
                && document
                    .get_span_content(&t.span)
                    .first()
                    .is_some_and(|c| c.is_lowercase())
                && self
                    .lemmas(&lowercase_of(t, document))
                    .iter()
                    // *rufe … an*: the verb without its particle, which the clause ends with.
                    .any(|lemma| {
                        lemma == verb || (verb.ends_with(lemma.as_str()) && lemma.len() > 3)
                    })
        };
        let mut lints = Vec::new();
        for (at, token) in clause.iter().enumerate() {
            let written = document.get_span_content_str(&token.span);
            let Some(&(dative, accusative)) = [("mir", "mich"), ("dir", "dich"), ("ihm", "ihn")]
                .iter()
                .find(|(d, _)| written == *d)
            else {
                continue;
            };
            if !at.checked_sub(1).is_some_and(|i| is_verb(clause[i])) {
                continue;
            }
            lints.extend(Self::report(
                token,
                vec![accusative.to_owned()],
                format!("»{verb}« verlangt den Akkusativ: »{accusative}«, nicht »{dative}«."),
            ));
        }
        lints
    }

    /// *Ich gebe ihn das Buch*: a verb of giving with a person and a thing takes the person in
    /// the dative. The pronoun has to stand right behind the verb and an article of the
    /// accusative right behind it, so *ich gebe ihn dem Mann* (the thing first) stays.
    fn indirect_object(
        &self,
        clause: &[&Token],
        verbs: &[String],
        document: &Document,
    ) -> Vec<Lint> {
        const GIVING_VERBS: &[&str] = &[
            "geben",
            "schenken",
            "zeigen",
            "schicken",
            "bringen",
            "erklären",
            "leihen",
            "empfehlen",
            "zurückgeben",
            "mitbringen",
            "verkaufen",
            "anbieten",
        ];
        const GIVING_FORMS: &[&str] = &[
            "gibt", "gibst", "gab", "gaben", "brachte", "brachten", "bot", "boten",
        ];
        let Some(verb) = verbs.iter().find(|v| GIVING_VERBS.contains(&v.as_str())) else {
            return Vec::new();
        };
        let is_verb = |t: &Token| {
            let word = lowercase_of(t, document);
            t.kind.is_word()
                && (GIVING_FORMS.contains(&word.as_str())
                    || self.lemmas(&word).iter().any(|lemma| {
                        lemma == verb || (verb.ends_with(lemma.as_str()) && lemma.len() > 3)
                    }))
        };
        let mut lints = Vec::new();
        for at in 1..clause.len().saturating_sub(1) {
            let written = document.get_span_content_str(&clause[at].span);
            let Some(&(dative, accusative)) =
                DATIVE_TO_ACCUSATIVE.iter().find(|(_, a)| written == *a)
            else {
                continue;
            };
            let thing_follows = matches!(
                lowercase_of(clause[at + 1], document).as_str(),
                "das"
                    | "den"
                    | "einen"
                    | "ein"
                    | "eine"
                    | "die"
                    | "meinen"
                    | "seinen"
                    | "ihren"
                    | "unseren"
                    | "keinen"
                    | "kein"
                    | "keine"
            );
            if is_verb(clause[at - 1]) && thing_follows {
                lints.extend(Self::report(
                    clause[at],
                    vec![dative.to_owned()],
                    format!(
                        "Bei »{verb}« steht die Person im Dativ: »{dative}«, nicht »{accusative}«."
                    ),
                ));
            }
        }
        lints
    }

    /// A direction after a verb of putting, in the dative: *Ich lege das Buch
    /// auf dem Tisch* → *auf den Tisch*. *hängen* and *stecken* are left out,
    /// for they are verbs of position too (*Das Bild hängt an der Wand*). A
    /// separable particle at the end of the clause makes it another verb
    /// (*Er stellt sich in dem Raum vor*), and the place is then a place.
    fn direction(&self, clause: &[&Token], verbs: &[String], document: &Document) -> Vec<Lint> {
        const DIRECTION_VERBS: &[&str] = &["legen", "stellen", "setzen", "werfen"];
        const PARTICLES: &[&str] = &[
            "vor", "ab", "an", "fest", "ein", "aus", "um", "dar", "her", "hin", "zusammen", "auf",
            "zu", "nach", "bei", "mit", "fort", "weg", "zurück", "bereit", "dazu", "nahe", "frei",
            "instand", "wieder",
        ];
        let Some(verb) = verbs.iter().find(|v| DIRECTION_VERBS.contains(&v.as_str())) else {
            return Vec::new();
        };
        // The last word before the next punctuation in the document: the
        // clause may have been cut at a conjunction or an article before it.
        let last_word = clause.first().and_then(|first| {
            document
                .get_tokens()
                .iter()
                .skip_while(|t| t.span.start < first.span.start)
                .take_while(|t| {
                    !matches!(
                        t.kind,
                        TokenKind::Punctuation(
                            Punctuation::Comma
                                | Punctuation::Period
                                | Punctuation::Semicolon
                                | Punctuation::Colon
                                | Punctuation::Question
                                | Punctuation::Bang
                        )
                    )
                })
                .filter(|t| t.kind.is_word())
                .last()
        });
        if last_word.is_some_and(|t| PARTICLES.contains(&lowercase_of(t, document).as_str())) {
            return Vec::new();
        }
        let is_direction_verb = |t: &Token| {
            t.kind.is_word()
                && document
                    .get_span_content(&t.span)
                    .first()
                    .is_some_and(|c| c.is_lowercase())
                && self.lemmas(&lowercase_of(t, document)).contains(verb)
        };
        let mut lints = Vec::new();
        for (at, token) in clause.iter().enumerate() {
            let preposition = lowercase_of(token, document);
            if !TWO_WAY_PREPOSITIONS.contains(&preposition.as_str()) {
                continue;
            }
            // The object between verb and preposition: *lege das Buch auf*.
            if !(1..=4).any(|back| {
                at.checked_sub(back)
                    .is_some_and(|i| is_direction_verb(clause[i]))
            }) {
                continue;
            }
            // *vor allem*: a fixed phrase, not a direction.
            let Some(object) = clause
                .get(at + 1)
                .filter(|t| t.kind.is_word())
                .filter(|t| !matches!(lowercase_of(t, document).as_str(), "allem" | "allen"))
            else {
                continue;
            };
            let noun = clause[at + 2..]
                .iter()
                .take(3)
                .find(|t| {
                    document
                        .get_span_content(&t.span)
                        .first()
                        .is_some_and(|c| c.is_uppercase())
                })
                .or(clause.get(at + 2));
            lints.extend(self.wrong_determiner(
                object,
                noun,
                Case::Accusative,
                document,
                || format!("Nach »{verb}« steht die Richtung im Akkusativ: »{preposition}« mit Akkusativ."),
            ));
        }
        lints
    }

    /// *Ich freue mir* → *mich*: verbs whose reflexive pronoun is accusative.
    /// And *Ich habe mich die Hände gewaschen* → *mir*: with a body part as the
    /// accusative object, the reflexive is the dative.
    fn reflexive(clause: &[&Token], verbs: &[String], document: &Document) -> Vec<Lint> {
        const ACCUSATIVE_REFLEXIVES: &[&str] = &[
            "freuen",
            "beeilen",
            "erinnern",
            "kümmern",
            "verlieben",
            "schämen",
            "wundern",
            "bedanken",
            "entschuldigen",
            "verabschieden",
            "konzentrieren",
            "gewöhnen",
            "beschweren",
            "bewerben",
            "verlassen",
            "fürchten",
            "interessieren",
            "ärgern",
            "langweilen",
            "erkälten",
            "verspäten",
            "irren",
            "verabreden",
        ];
        const BODY_PARTS: &[&str] = &[
            "Hände", "Hand", "Zähne", "Haare", "Bein", "Arm", "Gesicht", "Fuß", "Füße", "Kopf",
            "Nase", "Finger", "Knie", "Rücken", "Augen", "Ohren",
        ];
        let mut lints = Vec::new();
        let has = |w: &str| clause.iter().any(|t| lowercase_of(t, document) == w);
        let accusative_verb = verbs
            .iter()
            .any(|v| ACCUSATIVE_REFLEXIVES.contains(&v.as_str()));
        for (at, token) in clause.iter().enumerate() {
            let written = document.get_span_content_str(&token.span);
            let body_part_follows = clause.get(at + 1).is_some_and(|t| {
                matches!(lowercase_of(t, document).as_str(), "die" | "den" | "das")
            }) && clause.get(at + 2).is_some_and(|t| {
                BODY_PARTS.contains(&document.get_span_content_str(&t.span).as_str())
            });
            let (fix, why) = match written.as_str() {
                "mir" if accusative_verb && has("ich") && !body_part_follows => {
                    ("mich", "den Akkusativ")
                }
                "dir" if accusative_verb && has("du") && !body_part_follows => {
                    ("dich", "den Akkusativ")
                }
                "mich" if has("ich") && body_part_follows => (
                    "mir",
                    "den Dativ, weil der Körperteil das Akkusativobjekt ist",
                ),
                "dich" if has("du") && body_part_follows => (
                    "dir",
                    "den Dativ, weil der Körperteil das Akkusativobjekt ist",
                ),
                _ => continue,
            };
            lints.extend(Self::report(
                token,
                vec![fix.to_owned()],
                format!(
                    "Das Reflexivpronomen steht hier im Fall {why}: »{fix}«, nicht »{written}«."
                ),
            ));
        }
        lints
    }

    /// A place after a verb of position, in the accusative: *die Brille liegt
    /// auf den Tisch*.
    fn position(&self, clause: &[&Token], verbs: &[String], document: &Document) -> Vec<Lint> {
        let Some(verb) = verbs.iter().find(|v| POSITION_VERBS.contains(&v.as_str())) else {
            return Vec::new();
        };
        let is_position_verb = |t: &Token| {
            t.kind.is_word()
                && document
                    .get_span_content(&t.span)
                    .first()
                    .is_some_and(|c| c.is_lowercase())
                && self.lemmas(&lowercase_of(t, document)).contains(verb)
        };
        let mut lints = Vec::new();
        for (at, token) in clause.iter().enumerate() {
            let preposition = lowercase_of(token, document);
            if !TWO_WAY_PREPOSITIONS.contains(&preposition.as_str()) {
                continue;
            }
            // Right behind the verb, or one or two words on (*liegt jetzt auf*,
            // *befindet sich auf*): further away the phrase may belong to a noun,
            // *das Abtauchen unter den Block*.
            if !(1..=3).any(|back| {
                at.checked_sub(back)
                    .is_some_and(|i| is_position_verb(clause[i]))
            }) {
                continue;
            }
            let Some(object) = clause.get(at + 1).filter(|t| t.kind.is_word()) else {
                continue;
            };
            // *in eine kleine Wohnung*: the noun may stand behind adjectives.
            let noun = clause[at + 2..]
                .iter()
                .take(3)
                .find(|t| {
                    document
                        .get_span_content(&t.span)
                        .first()
                        .is_some_and(|c| c.is_uppercase())
                })
                .or(clause.get(at + 2));
            lints.extend(
                self.wrong_determiner(object, noun, Case::Dative, document, || {
                    format!("Nach »{verb}« steht der Ort im Dativ: »{preposition}« mit Dativ.")
                }),
            );
        }
        lints
    }
}

fn accusative_verbs(preposition: &str) -> Option<(&'static [&'static str], bool)> {
    ACCUSATIVE_OBJECTS
        .iter()
        .find(|(p, _, _)| *p == preposition)
        .map(|(_, verbs, determiners)| (*verbs, *determiners))
}

fn dative_verbs(preposition: &str) -> Option<(&'static [&'static str], bool)> {
    DATIVE_OBJECTS
        .iter()
        .find(|(p, _)| *p == preposition)
        .map(|(_, verbs)| (*verbs, true))
}

fn case_name(case: Case) -> &'static str {
    match case {
        Case::Nominative => "Nominativ",
        Case::Accusative => "Akkusativ",
        Case::Dative => "Dativ",
        Case::Genitive => "Genitiv",
    }
}

impl Linter for GermanVerbCase {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for sentence in document.iter_sentences() {
            let words: Vec<&Token> = sentence
                .iter()
                .filter(|token| !token.kind.is_whitespace())
                .collect();
            for clause in clauses(&words, document) {
                let verbs = self.verbs_of(&clause, document);
                if verbs.is_empty() {
                    continue;
                }
                lints.extend(self.prepositional_object(&clause, &verbs, document));
                lints.extend(self.dative_object(&clause, &verbs, document));
                lints.extend(self.accusative_object(&clause, &verbs, document));
                lints.extend(self.indirect_object(&clause, &verbs, document));
                lints.extend(self.position(&clause, &verbs, document));
                lints.extend(self.direction(&clause, &verbs, document));
                lints.extend(Self::reflexive(&clause, &verbs, document));
            }
        }
        lints.sort_by_key(|lint| lint.span.start);
        lints.dedup_by_key(|lint| lint.span);
        lints
    }

    fn description(&self) -> &str {
        "Prüft den Fall, den ein Verb verlangt: »ich erinnere mich an dich«, »er hilft seinem Freund«, »die Brille liegt auf dem Tisch«."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanVerbCase;
    use crate::parsers::PlainGerman;
    use crate::spell::compound_aware_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn fixes(text: &str) -> Vec<(String, Vec<String>)> {
        let dictionary = compound_aware_german_dictionary();
        let document = Document::new(text, &PlainGerman, &*dictionary);
        GermanVerbCase::default()
            .lint(&document)
            .iter()
            .map(|lint| {
                (
                    document.get_span_content_str(&lint.span),
                    lint.suggestions
                        .iter()
                        .map(|s| {
                            s.to_string()
                                .replace("Replace with: ", "")
                                .replace(['“', '”'], "")
                        })
                        .collect(),
                )
            })
            .collect()
    }

    fn fixed(text: &str) -> Vec<String> {
        fixes(text).into_iter().map(|(w, _)| w).collect()
    }

    #[test]
    fn catches_the_dative_where_the_verb_wants_the_accusative() {
        assert_eq!(fixed("Ich erinnere mich an dir."), ["dir"]);
        assert_eq!(fixed("Ich denke oft an ihm."), ["ihm"]);
        assert_eq!(fixed("Wir warten auf dir."), ["dir"]);
        assert_eq!(fixed("Ich denke an meinem Vater."), ["meinem"]);
        assert_eq!(fixed("Sie freut sich über dem Geschenk."), ["dem"]);
        assert_eq!(fixes("Sie freut sich über dem Geschenk.")[0].1, ["das"]);
    }

    #[test]
    fn catches_the_accusative_where_the_verb_wants_the_dative() {
        assert_eq!(fixed("Er hat mich geholfen."), ["mich"]);
        assert_eq!(fixed("Ich danke dich."), ["dich"]);
        assert_eq!(
            fixed("Er beteuerte, dass es keine Absicht war, jemanden zu schaden."),
            ["jemanden"]
        );
        assert_eq!(
            fixes("Er beteuerte, dass es keine Absicht war, jemanden zu schaden.")[0].1,
            ["jemandem"]
        );
        assert_eq!(fixed("Er wollte niemanden schaden."), ["niemanden"]);
        assert_eq!(fixed("Sie hat jemanden geholfen."), ["jemanden"]);
        assert_eq!(fixed("Er wohnt in eine kleine Wohnung."), ["eine"]);
        assert_eq!(fixed("Ich danke Sie für Ihre Hilfe."), ["Sie"]);
        assert_eq!(fixed("Er fragt mir nach dem Weg."), ["mir"]);
        assert_eq!(fixed("Ich kenne ihm nicht."), ["ihm"]);
        assert_eq!(fixed("Ich gebe ihn das Buch morgen zurück."), ["ihn"]);
        assert_eq!(fixed("Er zeigt mich den Weg."), ["mich"]);
        assert_eq!(fixed("Ich frage ihm nach dem Weg."), ["ihm"]);
        assert_eq!(fixed("Ich rufe dir morgen an."), ["dir"]);
        assert_eq!(fixed("Das Buch gehört mich."), ["mich"]);
        assert_eq!(fixed("Ich glaube dich nicht."), ["dich"]);
        assert_eq!(fixed("Ich interessiere mir für Kunst."), ["mir"]);
        assert_eq!(fixed("Ich lege das Buch auf dem Tisch."), ["dem"]);
        assert_eq!(fixed("Er stellt die Vase in dem Schrank."), ["dem"]);
        assert_eq!(fixed("Ich freue mir auf den Urlaub."), ["mir"]);
        assert_eq!(fixed("Ich habe mich die Hände gewaschen."), ["mich"]);
        assert_eq!(fixed("Ich nehme an den Kurs teil."), ["den"]);
        assert_eq!(fixed("Er fürchtet sich vor den Hund."), ["den"]);
        assert_eq!(fixed("Die Brille liegt auf den Tisch."), ["den"]);
        assert_eq!(fixed("Das Buch liegt unter einen Stuhl."), ["einen"]);
    }

    #[test]
    fn leaves_the_right_case_and_the_places_alone() {
        for text in [
            "Ich erinnere mich an dich.",
            "Ich warte auf dem Bahnsteig auf den Zug.",
            "Er freute sich auf dem Balkon über das Wetter.",
            "An dem Tag dachte er an seine Mutter.",
            "Ich denke an dem Tag an dich.",
            "Lass mich helfen.",
            "Er hilft den Kindern.",
            "Er hilft mir, den Koffer zu tragen.",
            "Er bat jemanden zu helfen.",
            "Er bat mich zu helfen.",
            "Es war keine Absicht, jemandem zu schaden.",
            "Er hat niemandem geholfen.",
            "Er sah jemanden helfen.",
            "Jemanden zu finden, der hilft, ist schwer.",
            "Danken Sie ihm.",
            "Er wohnt in einer kleinen Wohnung.",
            "Er fragt mich nach dem Weg.",
            "Ich glaube ihn zu kennen.",
            "Ich glaube dir nicht.",
            "Das gehört mir.",
            "Ich kenne ihn nicht.",
            "Ich treffe ihn morgen.",
            "Er hat ihm die Hand gegeben.",
            "Ich sehe ihm beim Spielen zu.",
            "Ich rufe dich morgen an.",
            "Ich gebe ihn dem Mann.",
            "Ich gebe ihm das Buch.",
            "Er bringt mich nach Hause.",
            "Kannst du mir sagen, was er fragt?",
            "Er fragt sich, ob es stimmt.",
            "Sie hängt das Bild an die Wand.",
            "Das Bild hängt an der Wand.",
            "Ich lege das Buch auf den Tisch.",
            "Er stellt sich in dem Raum vor.",
            "Der Kaiser legte ihm in einem Gespräch den Rücktritt nahe.",
            "Man setzte es vor allem während des Krieges ein.",
            "Ich freue mich auf den Urlaub.",
            "Ich habe mir die Hände gewaschen.",
            "Ich kaufe mir ein Auto.",
            "Kannst du mir sagen, ob ich mich freuen soll?",
            "Ich nehme an dem Kurs teil.",
            "Die Brille liegt auf dem Tisch.",
            "Er legt die Brille auf den Tisch.",
            "Sie wohnen in den USA.",
            "Ich stehe auf den Typen.",
            "Er dankt den Helfern.",
            "Wir sprechen über den Film.",
            "Er arbeitet an den Plänen.",
            "Dazu gehören das Sieb und die Waage.",
            "Es folgt eine Übersicht.",
            "Diese Folgen des Krieges sind schlimm.",
            "Er nahm das Leben in die Hand.",
            "In Anlehnung an die Arbeiten von Weber.",
            "Durch das Abtauchen unter den Block befindet sich dort ein Graben.",
        ] {
            assert!(fixes(text).is_empty(), "{text}: {:?}", fixes(text));
        }
    }
}
