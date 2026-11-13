use crate::grammar::determiners::determiner_readings;
use crate::grammar::noun_phrase;
use crate::grammar::verbs::{INFINITIVE_GROUP_OPENERS, has_zu_infix};
use crate::linting::german_relative_clause_comma::GermanRelativeClauseComma;
use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::{Document, Punctuation, Token, TokenKind, TokenStringExt};

/// Conjunctions that always open a subordinate clause, and therefore always take
/// a comma in front of them.
///
/// Deliberately a short list. `während`, `wenn`, `als` and `damit` are left out
/// because each is also something else — a preposition, part of `wenn auch`, a
/// comparison, a pronominal adverb — and telling them apart needs more than the
/// word itself.
pub(crate) const SUBORDINATORS: &[&str] = &[
    // `dass` is the least ambiguous of them all: unlike `das` it is never a
    // pronoun and never an article, so it opens a subordinate clause every
    // time it appears. Comma before `dass` is also the most common comma
    // mistake in written German.
    "dass",
    "weil",
    "obwohl",
    "obgleich",
    "obschon",
    "sodass",
    "falls",
    "sobald",
    "solange",
    "nachdem",
    "bevor",
    "zumal",
    "sofern",
    "wohingegen",
    // `ob` opens the indirect question and nothing else in modern German —
    // the preposition (*ob seiner Verdienste*) is archaic. The words that fuse
    // with it, *als ob*, *egal ob*, are in [`OB_MODIFIERS`].
    "ob",
];

/// Words that may stand between the comma and the conjunction, carrying the
/// comma further to the left: *"Er kam, **vor allem** weil es regnete"*.
const FOCUS_PARTICLES: &[&str] = &[
    // focus particles
    "allem",
    "gerade",
    "nur",
    "auch",
    "besonders",
    "eben",
    "schon",
    "erst",
    "insbesondere",
    "vor",
    "selbst",
    "sogar",
    "immer",
];

/// Coordinators, where the comma question is settled rather than moved.
///
/// *»…, und weil er krank war, rief er an«* needs its comma in front of `und`,
/// and *»Sie unterscheiden sich von Komposita … und weil sie den Wortakzent
/// verlieren«* needs none at all — either way the conjunction is not what the
/// comma would separate. So the walk **stops** here and accepts, where at a
/// particle it carries on. Treating these as particles to walk through is what
/// made the first version of this check report five clauses that were already
/// correct.
const CLAUSE_COORDINATORS: &[&str] = &["und", "oder", "aber", "sondern", "denn", "doch"];

/// `je nachdem` is a fixed phrase — "depending" — and not a subordinate clause
/// at all, so the question of a comma does not arise. It sat in
/// [`FOCUS_PARTICLES`] before, which worked only while the check looked exactly
/// one token to the left: walking further finds the verb in "Die Städte haben je
/// nachdem einen …" and asks for a comma again.
const FIXED_PHRASES: &[(&str, &str)] = &[("je", "nachdem")];

/// Modifiers that fuse with a *temporal* conjunction — "noch bevor", "kurz
/// nachdem", "unmittelbar bevor" — where the comma goes in front of the pair.
///
/// They are kept apart from [`FOCUS_PARTICLES`] because they do not generalize:
/// "Das Haus steht noch, obwohl es alt ist" needs its comma, and treating `noch`
/// as a particle everywhere would swallow it.
const TEMPORAL_MODIFIERS: &[&str] = &[
    "noch",
    "kurz",
    "lange",
    "unmittelbar",
    "direkt",
    "gleich",
    "kaum",
    "erst",
    "schon",
];

/// The conjunctions [`TEMPORAL_MODIFIERS`] may attach to.
const TEMPORAL_SUBORDINATORS: &[&str] = &["bevor", "nachdem", "sobald", "solange"];

/// Words that fuse with `dass` into a two-part conjunction — *ohne dass*,
/// *statt dass*, *auf dass*, *als dass*, *kaum dass*, *so dass*.
///
/// The comma goes in front of the pair, and the pair is what governs the
/// clause. Suggesting one after the first half would produce *"ging ohne, dass
/// jemand es merkte"*, which is wrong; the sentence wants *"ging, ohne dass"*.
const DASS_MODIFIERS: &[&str] = &[
    "ohne",
    "statt",
    "anstatt",
    "außer",
    "auf",
    "als",
    "kaum",
    "so",
    "angenommen",
    "vorausgesetzt",
    "gesetzt",
    "geschweige",
];

/// Words that fuse with `ob` into one conjunction, taking the comma in front
/// of the pair: *», als ob nichts gewesen wäre«*, *», egal ob es regnet«*,
/// *», je nachdem ob…«*.
const OB_MODIFIERS: &[&str] = &[
    "als",
    "egal",
    "gleich",
    "gleichgültig",
    "einerlei",
    "unabhängig",
    "nachdem",
    "wie",
];

/// Conjunctions that open a subordinate clause at the start of a sentence,
/// whose end then needs a comma before the main clause's verb: *Wenn du
/// kommst, bringe Brot mit*. *Da* (also an adverb) and *während* (also a
/// preposition) are left out.
const SENTENCE_INITIAL_SUBORDINATORS: &[&str] = &[
    "wenn", "weil", "als", "obwohl", "dass", "nachdem", "bevor", "falls", "ob", "sobald",
    "solange", "seitdem", "indem", "sofern",
];

/// Requires the comma German grammar requires in front of a subordinate clause.
///
/// Comma placement is the most common mistake in written German, and most of it
/// needs a parser. This part does not: these conjunctions open a subordinate
/// clause and nothing else, so a comma belongs in front of every one of them
/// that does not start a sentence.
#[derive(Default)]
pub struct GermanSubordinateComma;

impl GermanSubordinateComma {
    /// Is this the conjunction, rather than a proper name spelled the same?
    ///
    /// A conjunction is lower case mid-sentence. `Weil` with a capital is
    /// Stephan Weil, or the place — the corpus has "im Kabinett Weil III".
    fn is_subordinator(token: &Token, document: &Document) -> bool {
        let chars = document.get_span_content(&token.span);
        if chars.first().is_some_and(|c| c.is_uppercase()) {
            return false;
        }
        let word: String = chars.iter().collect();
        SUBORDINATORS.contains(&word.as_str())
    }

    fn is_focus_particle(token: &Token, document: &Document) -> bool {
        Self::word_in(token, document, FOCUS_PARTICLES)
    }

    /// Does a comma already stand to the left, with only particles in between?
    ///
    /// The comma belongs in front of the material that governs the clause, not
    /// in front of the conjunction: *», wohl weil man…«*, *», teils weil es…«*,
    /// *», einfach weil der Berg da ist«*, *», vermutlich weil…«*, *», etwa
    /// weil er…«*. Looking one token to the left reports every one of those as a
    /// missing comma — eleven of the thirteen this rule produced on a corpus of
    /// published German prose.
    ///
    /// Walking instead of listing is the point. [`FOCUS_PARTICLES`] had the
    /// right idea and could only ever hold the particles somebody thought of;
    /// what actually licenses the comma being further left is that everything
    /// between it and the conjunction modifies the clause rather than being part
    /// of one. A verb or a noun in between means a clause of its own, and the
    /// comma really is missing.
    fn comma_is_further_left(tokens: &[&Token], index: usize, document: &Document) -> bool {
        let mut cursor = index;
        while let Some(previous) = cursor.checked_sub(1).map(|i| tokens[i]) {
            if !matches!(previous.kind, TokenKind::Word(_)) {
                // Punctuation: a comma, dash, colon or bracket separates the
                // clauses and anything else does not.
                return true;
            }
            if Self::word_in(previous, document, CLAUSE_COORDINATORS) {
                return true;
            }
            // *», zum Beispiel weil…«*: a fixed phrase that modifies the clause,
            // although `Beispiel` is a noun.
            if Self::word_in(previous, document, &["beispiel"])
                && cursor >= 2
                && Self::word_in(tokens[cursor - 2], document, &["zum"])
            {
                cursor -= 2;
                continue;
            }
            if !Self::modifies_the_clause(previous, document) {
                return false;
            }
            cursor -= 1;
        }
        // Reaching the start of the sentence: nothing to separate.
        true
    }

    /// May this word stand between the comma and the conjunction?
    ///
    /// Anything that can be read as an adverb or an adjective, and nothing that
    /// can only be a noun or a verb. Those cannot open a clause of their own, so
    /// a comma in front of them is the comma this rule is looking for.
    ///
    /// A word with *both* readings does not count, even though `teils` is an
    /// adverb as well as the genitive of `Teil` and is the adverb in *», teils
    /// weil es Dokumentationen gibt«*. Accepting any adverb reading was tried
    /// and is a bad trade: German separable prefixes are adverbs too, so *»Sie
    /// rief an nachdem sie angekommen war«* walks past `an` to a verb it should
    /// have stopped at. Two corpus false positives fewer, four real missing
    /// commas missed.
    fn modifies_the_clause(token: &Token, document: &Document) -> bool {
        if Self::is_focus_particle(token, document) {
            return true;
        }
        let TokenKind::Word(Some(metadata)) = &token.kind else {
            // An unknown word could be anything, including a noun.
            return false;
        };
        !metadata.is_noun() && !metadata.is_verb() && !metadata.is_proper_noun()
    }

    /// Is the conjunction being *talked about* rather than used?
    ///
    /// German writes that as a hyphenated compound — *»den obwohl-Satz«*, *»die
    /// weil-Konstruktion«* — and a linguistics article is full of them.
    fn is_hyphenated_mention(tokens: &[&Token], index: usize) -> bool {
        tokens
            .get(index + 1)
            .is_some_and(|next| matches!(next.kind, TokenKind::Punctuation(Punctuation::Hyphen)))
    }

    /// Is the conjunction named rather than used, without a hyphen to show it?
    ///
    /// A grammar article lists them: *»Subjunktionen sind vor allem dass
    /// (früher daß geschrieben)«*, *»können dass und ob nur mit finiten
    /// Nebensätzen«*, *»Inhaltssätze mit dass oder ob«*. A conjunction in use
    /// is followed by the clause it opens, so what follows settles it — a
    /// coordinator or a bracket means the word is one item in a list of words,
    /// not the start of a sentence. A second conjunction in front of it says
    /// the same: *»wogegen dass vor allem Aussagen markiert«* has two of them
    /// in a row, which no German clause does.
    fn is_bare_mention(tokens: &[&Token], index: usize, document: &Document) -> bool {
        const WORD_LIST_JOINERS: &[&str] = &["und", "oder", "bzw", "beziehungsweise", "sowie"];
        const OTHER_CONJUNCTIONS: &[&str] = &[
            "ob", "wogegen", "während", "wenn", "wie", "wo", "wobei", "womit", "wodurch",
        ];

        let next_is_a_joiner = tokens.get(index + 1).is_some_and(|next| {
            !matches!(next.kind, TokenKind::Word(_))
                || Self::word_in(next, document, WORD_LIST_JOINERS)
        });
        let previous_is_a_conjunction = index.checked_sub(1).is_some_and(|i| {
            Self::word_in(tokens[i], document, OTHER_CONJUNCTIONS)
                || Self::word_in(tokens[i], document, SUBORDINATORS)
        });

        next_is_a_joiner || previous_is_a_conjunction
    }

    /// *Wenn du kommst bringe Brot mit*: a sentence that opens with a
    /// subordinate clause puts the main clause's verb right behind the
    /// subordinate clause's, and the comma goes between them.
    ///
    /// Two adjacent verbs are the evidence, the same one
    /// `GermanRelativeClauseComma` reads: the subordinate clause is verb-final,
    /// the main clause verb-first, so where a run of verbs is followed by more
    /// of the sentence, its last verb is the main clause's.
    fn missing_comma_after_initial_clause(tokens: &[&Token], document: &Document) -> Option<Lint> {
        let first = tokens.first()?;
        // *Wo du bist, ist mir egal*, *Was du sagst, stimmt*: a question word opens a
        // subordinate clause when a subject follows it and the sentence asks nothing.
        let question_word_clause = Self::word_in(first, document, &["wo", "was", "wer", "wie"])
            && tokens.get(1).is_some_and(|next| {
                Self::word_in(
                    next,
                    document,
                    &["ich", "du", "er", "sie", "wir", "ihr", "man"],
                ) || (Self::word_in(next, document, &["das", "der", "die", "es"])
                    && first.span.start == 0)
            })
            && !tokens.last().is_some_and(|last| {
                matches!(last.kind, TokenKind::Punctuation(Punctuation::Question))
            });
        if !matches!(first.kind, TokenKind::Word(_))
            || !(Self::word_in(first, document, SENTENCE_INITIAL_SUBORDINATORS)
                || question_word_clause)
        {
            return None;
        }
        let (last, main_clause_continues) =
            GermanRelativeClauseComma::clause_end(tokens, 0, document)?;
        if !main_clause_continues {
            return None;
        }
        // The clause needs a subject in front of its verbs: *Dass essen gehen
        // ein Luxus ist* has an infinitive phrase where the verbs would be.
        // And a conjunction behind the verbs continues the subordinate clause
        // rather than starting the main one: *Wenn du Schluss machen willst
        // bevor es angefangen hat*.
        let first_verb = (1..=last + 1)
            .find(|&at| GermanRelativeClauseComma::is_verb_token(tokens[at], document))?;
        if first_verb < 2 {
            return None;
        }
        if tokens
            .get(last + 2)
            .is_some_and(|after| Self::word_in(after, document, SUBORDINATORS))
        {
            return None;
        }
        let conjunction: String = document.get_span_content(&first.span).iter().collect();
        Some(Lint {
            span: tokens[last].span,
            lint_kind: LintKind::Punctuation,
            suggestions: vec![Suggestion::InsertAfter(vec![','])],
            priority: 28,
            message: format!(
                "Der mit »{conjunction}« eingeleitete Nebensatz endet hier. Danach steht ein Komma."
            ),
        })
    }

    /// *Statt zu arbeiten schläft er*, *Um zu gewinnen muss man trainieren*: an infinitive
    /// group that opens the sentence ends at its infinitive, and the main clause's verb follows.
    fn missing_comma_after_initial_infinitive_group(
        tokens: &[&Token],
        document: &Document,
    ) -> Option<Lint> {
        let first = tokens.first()?;
        if !Self::word_in(first, document, INFINITIVE_GROUP_OPENERS) {
            return None;
        }
        // The infinitive: the word behind *zu*, or one with its *zu* inside.
        let infinitive = (1..tokens.len()).find(|&at| {
            let word = noun_phrase::lowercase_of(tokens[at], document);
            has_zu_infix(&word)
                || (at > 1 && noun_phrase::lowercase_of(tokens[at - 1], document) == "zu")
        })?;
        if tokens[1..infinitive]
            .iter()
            .any(|t| matches!(t.kind, TokenKind::Punctuation(_)))
        {
            return None;
        }
        let next = tokens.get(infinitive + 1)?;
        let next_word = noun_phrase::lowercase_of(next, document);
        let finite = matches!(next.kind, TokenKind::Word(_))
            && document
                .get_span_content(&next.span)
                .first()
                .is_some_and(|c| c.is_lowercase())
            && (GermanRelativeClauseComma::is_verb_token(next, document)
                || crate::grammar::subjects::irregular_finite_verb_lemma(&next_word).is_some())
            && !next.kind.is_adjective()
            && !next.kind.is_adverb()
            && next_word != "und";
        finite.then(|| Lint {
            span: tokens[infinitive].span,
            lint_kind: LintKind::Punctuation,
            suggestions: vec![Suggestion::InsertAfter(vec![','])],
            priority: 28,
            message: "Die Infinitivgruppe am Satzanfang endet hier. Danach steht ein Komma."
                .to_owned(),
        })
    }

    /// *Ja ich komme*, *Nein das geht nicht*, *Ach so das wusste ich nicht*: an answer particle
    /// that opens the sentence is set off by a comma.
    fn missing_comma_after_answer_particle(tokens: &[&Token], document: &Document) -> Option<Lint> {
        let first = tokens.first()?;
        let first_word = noun_phrase::lowercase_of(first, document);
        let (particle_end, after) = match first_word.as_str() {
            "ja" | "nein" | "okay" | "ok" | "tja" => (0, 1),
            "ach"
                if tokens
                    .get(1)
                    .is_some_and(|t| noun_phrase::lowercase_of(t, document) == "so") =>
            {
                (1, 2)
            }
            _ => return None,
        };
        let next = tokens.get(after)?;
        if !matches!(next.kind, TokenKind::Word(_)) {
            return None;
        }
        // *Ja doch*, *Nein danke*, *Ja gerne*, *Ja klar*: the particle with its own reinforcement.
        if Self::word_in(
            next,
            document,
            &[
                "ja",
                "nein",
                "doch",
                "danke",
                "bitte",
                "gerne",
                "gern",
                "klar",
                "genau",
                "sicher",
                "natürlich",
                "wohl",
                "schon",
                "eben",
                "aber",
                "so",
                "und",
                "oder",
                "mal",
            ],
        ) {
            return None;
        }
        let is_subject_or_verb = Self::word_in(
            next,
            document,
            &[
                "ich", "du", "er", "sie", "es", "wir", "ihr", "man", "das", "der", "die", "dies",
                "da", "dann",
            ],
        ) || GermanRelativeClauseComma::is_verb_token(next, document);
        is_subject_or_verb.then(|| Lint {
            span: tokens[particle_end].span,
            lint_kind: LintKind::Punctuation,
            suggestions: vec![Suggestion::InsertAfter(vec![','])],
            priority: 28,
            message: "Nach einer Antwortpartikel am Satzanfang steht ein Komma.".to_owned(),
        })
    }

    /// *Sie fragte mich wann wir losfahren*, *ich weiß nicht wo er wohnt*: an
    /// indirect question after a verb of asking or knowing takes a comma.
    ///
    /// The question words are relative pronouns and comparisons as well
    /// (*so wie*, *der Ort, wo*), so the verb has to come first, with at most
    /// an object pronoun or *nicht* between it and the question word. *was* is
    /// left out: *ich weiß was* is the colloquial *etwas*.
    fn missing_comma_before_indirect_question(tokens: &[&Token], document: &Document) -> Vec<Lint> {
        const ASKING: &[&str] = &[
            "frage",
            "fragst",
            "fragt",
            "fragte",
            "fragten",
            "gefragt",
            "fragen",
            "weiß",
            "weißt",
            "wisst",
            "wissen",
            "wusste",
            "wussten",
            "gewusst",
            "verstehe",
            "versteht",
            "verstand",
            "verstanden",
            "überlege",
            "überlegt",
            "überlegte",
            "entscheiden",
        ];
        const BETWEEN: &[&str] = &[
            "mich", "dich", "ihn", "sie", "uns", "euch", "nicht", "genau", "auch", "noch", "nie",
            "selbst", "schon", "es",
        ];
        const QUESTION_WORDS: &[&str] = &[
            "wann", "warum", "weshalb", "weswegen", "wieso", "woher", "wohin", "wo", "wer", "wen",
            "wem", "wessen", "wie", "welche", "welcher", "welches", "welchen", "womit", "wozu",
            "worüber", "wofür",
        ];
        let mut lints = Vec::new();
        for (at, token) in tokens.iter().enumerate() {
            if !Self::word_in(token, document, QUESTION_WORDS)
                || document
                    .get_span_content(&token.span)
                    .first()
                    .is_some_and(|c| c.is_uppercase())
            {
                continue;
            }
            let mut verb = at;
            while verb > 0 && Self::word_in(tokens[verb - 1], document, BETWEEN) {
                verb -= 1;
            }
            let Some(verb_at) = verb.checked_sub(1) else {
                continue;
            };
            if !matches!(tokens[verb_at].kind, TokenKind::Word(_))
                || !Self::word_in(tokens[verb_at], document, ASKING)
                || !matches!(tokens[at - 1].kind, TokenKind::Word(_))
            {
                continue;
            }
            // *wie* in *so wie*, *genauso wie*, *sowohl … wie*: a comparison.
            if Self::word_in(token, document, &["wie"])
                && tokens
                    .get(at + 1)
                    .is_none_or(|next| !matches!(next.kind, TokenKind::Word(_)))
            {
                continue;
            }
            // The question needs a clause: a verb somewhere behind the word.
            if !tokens[at + 1..]
                .iter()
                .take_while(|t| matches!(t.kind, TokenKind::Word(_)))
                .any(|t| GermanRelativeClauseComma::is_verb_token(t, document))
            {
                continue;
            }
            let question: String = document.get_span_content(&token.span).iter().collect();
            lints.push(Lint {
                span: tokens[at - 1].span,
                lint_kind: LintKind::Punctuation,
                suggestions: vec![Suggestion::InsertAfter(vec![','])],
                priority: 28,
                message: format!("Vor dem indirekten Fragesatz mit »{question}« steht ein Komma."),
            });
        }
        lints
    }

    /// *Er hat gesagt er kommt später*, *ich glaube er hat recht*: a main clause
    /// reported without *dass* takes a comma after the verb that reports it.
    ///
    /// The evidence is a subject pronoun right behind the verb of saying or
    /// thinking, followed by a finite verb: *er kommt*, *es gibt*. Without the
    /// verb behind the pronoun it is an object, *er sagte es ihm*, *ich glaube
    /// ihr kein Wort*.
    fn missing_comma_before_reported_clause(tokens: &[&Token], document: &Document) -> Vec<Lint> {
        const SAYING: &[&str] = &[
            "gesagt",
            "gemeint",
            "gedacht",
            "geglaubt",
            "gehofft",
            "behauptet",
            "sagte",
            "sagten",
            "sagt",
            "meinte",
            "meint",
            "dachte",
            "glaube",
            "glaubt",
            "glaubte",
            "hoffe",
            "hofft",
            "hoffte",
            "denke",
            "denkt",
            "finde",
            "findet",
            "behauptete",
            "schätze",
            "fürchte",
            "wette",
        ];
        const SUBJECTS: &[&str] = &["ich", "du", "er", "sie", "es", "wir", "ihr", "man"];
        let mut lints = Vec::new();
        for at in 0..tokens.len().saturating_sub(2) {
            let (verb, subject, finite) = (tokens[at], tokens[at + 1], tokens[at + 2]);
            if !matches!(verb.kind, TokenKind::Word(_))
                || !Self::word_in(verb, document, SAYING)
                || !matches!(subject.kind, TokenKind::Word(_))
                || !Self::word_in(subject, document, SUBJECTS)
                || !matches!(finite.kind, TokenKind::Word(_))
            {
                continue;
            }
            let finite_text: String = document.get_span_content(&finite.span).iter().collect();
            let is_finite = finite_text.chars().next().is_some_and(char::is_lowercase)
                && GermanRelativeClauseComma::is_verb_token(finite, document)
                && !finite.kind.is_noun()
                && !finite.kind.is_adjective();
            if !is_finite {
                continue;
            }
            lints.push(Lint {
                span: verb.span,
                lint_kind: LintKind::Punctuation,
                suggestions: vec![Suggestion::InsertAfter(vec![','])],
                priority: 28,
                message: "Der folgende Satz gibt wieder, was gesagt oder gedacht wird. Davor steht ein Komma."
                    .to_owned(),
            });
        }
        lints
    }

    /// *Sie haben gespielt bis es dunkel wurde*: *bis* opens a clause when a
    /// subject pronoun follows it, which no preposition takes. *bis morgen*,
    /// *bis zum Abend* stay quiet. The verb in front has to be a participle or
    /// an infinitive, so that *bis es* at the start of a sentence is not touched.
    fn missing_comma_before_bis_clause(tokens: &[&Token], document: &Document) -> Vec<Lint> {
        const SUBJECTS: &[&str] = &["ich", "du", "er", "sie", "es", "wir", "ihr", "man"];
        let mut lints = Vec::new();
        for at in 1..tokens.len().saturating_sub(2) {
            let (before, bis, subject) = (tokens[at - 1], tokens[at], tokens[at + 1]);
            if !Self::word_in(bis, document, &["bis"])
                || !matches!(before.kind, TokenKind::Word(_))
                || !Self::word_in(subject, document, SUBJECTS)
            {
                continue;
            }
            let written: String = document.get_span_content(&before.span).iter().collect();
            let participle = written.starts_with("ge")
                && (written.ends_with('t') || written.ends_with("en"))
                && written.chars().count() > 4;
            if !participle
                || before.kind.is_noun()
                || !GermanRelativeClauseComma::is_verb_token(before, document)
            {
                continue;
            }
            lints.push(Lint {
                span: before.span,
                lint_kind: LintKind::Punctuation,
                suggestions: vec![Suggestion::InsertAfter(vec![','])],
                priority: 28,
                message: "Vor dem Nebensatz mit »bis« steht ein Komma.".to_owned(),
            });
        }
        lints
    }

    /// *Das Ziel diese Werte zu erreichen*: an infinitive group that depends on
    /// a noun takes a comma (Amtliche Regelung § 75 (2)).
    ///
    /// Only the nouns that take an infinitive, only a group of more than the
    /// bare infinitive (*keine Lust zu arbeiten* may go without), and only when
    /// nothing in between could end the noun's clause: a comma, a finite verb
    /// or a conjunction sends the walk home empty.
    fn missing_comma_after_noun_with_infinitive(
        tokens: &[&Token],
        document: &Document,
    ) -> Vec<Lint> {
        const NOUNS: &[&str] = &[
            "ziel",
            "absicht",
            "aufgabe",
            "möglichkeit",
            "versuch",
            "plan",
            "idee",
            "wunsch",
            "gelegenheit",
            "fähigkeit",
            "pflicht",
            "mut",
            "lust",
            "chance",
            "entscheidung",
            "bereitschaft",
            "auftrag",
            "befehl",
            "erlaubnis",
            "gefahr",
            "angebot",
            "vorschlag",
            "forderung",
            "bitte",
            "hoffnung",
            "kraft",
            "methode",
            "versprechen",
            "vorhaben",
        ];
        let mut lints = Vec::new();
        for (at, token) in tokens.iter().enumerate() {
            let is_zu_infinitive = Self::word_in(token, document, &["zu"])
                && tokens.get(at + 1).is_some_and(|verb| {
                    let text = document.get_span_content_str(&verb.span);
                    text.chars().next().is_some_and(char::is_lowercase)
                        && text.ends_with('n')
                        && GermanRelativeClauseComma::is_verb_token(verb, document)
                })
                && tokens
                    .get(at + 2)
                    .is_none_or(|after| !matches!(after.kind, TokenKind::Word(_)));
            // *den Iran anzugreifen*: the *zu* inside a separable verb.
            let is_infix_infinitive = matches!(token.kind, TokenKind::Word(_))
                && has_zu_infix(&document.get_span_content_str(&token.span))
                && tokens
                    .get(at + 1)
                    .is_none_or(|after| !matches!(after.kind, TokenKind::Word(_)));
            if !is_zu_infinitive && !is_infix_infinitive {
                continue;
            }
            // Walk left over the group to the noun.
            let mut cursor = at;
            let mut words_between = 0;
            let noun_at = loop {
                let Some(previous) = cursor.checked_sub(1) else {
                    break None;
                };
                let t = tokens[previous];
                if !matches!(t.kind, TokenKind::Word(_)) {
                    break None;
                }
                if Self::takes_infinitive(t, document, NOUNS)
                    && document
                        .get_span_content(&t.span)
                        .first()
                        .is_some_and(|c| c.is_uppercase())
                {
                    break Some(previous);
                }
                let lower = document.get_span_content_str(&t.span).to_lowercase();
                let finite = GermanRelativeClauseComma::is_verb_token(t, document)
                    && document
                        .get_span_content(&t.span)
                        .first()
                        .is_some_and(|c| c.is_lowercase());
                if finite
                    || Self::word_in(t, document, CLAUSE_COORDINATORS)
                    || Self::word_in(t, document, SUBORDINATORS)
                    || matches!(lower.as_str(), "zu" | "um" | "ohne" | "statt" | "anstatt")
                {
                    break None;
                }
                words_between += 1;
                cursor = previous;
            };
            let Some(noun_at) = noun_at else {
                continue;
            };
            if words_between == 0 {
                continue;
            }
            // *aus einer Absicht heraus*: the noun belongs to a circumposition,
            // and the infinitive to the verb before it.
            const CIRCUMPOSITIONS: &[&str] = &[
                "heraus",
                "hinaus",
                "herum",
                "zufolge",
                "entgegen",
                "gegenüber",
            ];
            if tokens
                .get(noun_at + 1)
                .is_some_and(|after| Self::word_in(after, document, CIRCUMPOSITIONS))
            {
                continue;
            }
            // *um über die Möglichkeit zu diskutieren*, *ohne Gefahr eines
            // Irrtums zu entscheiden*: the infinitive belongs to *um* or *ohne*
            // further left in the clause.
            let clause_start = tokens[..noun_at]
                .iter()
                .rposition(|t| !matches!(t.kind, TokenKind::Word(_)))
                .map_or(0, |i| i + 1);
            if tokens[clause_start..noun_at]
                .iter()
                .any(|t| Self::word_in(t, document, &["um", "ohne", "statt", "anstatt"]))
            {
                continue;
            }
            // *…, die Entscheidung für die Schreibweise nachzuvollziehen*: the
            // noun is the object inside a group a comma has already opened.
            let behind_comma = clause_start.checked_sub(1).is_some_and(|i| {
                matches!(tokens[i].kind, TokenKind::Punctuation(Punctuation::Comma))
            });
            if behind_comma
                && !tokens[clause_start..noun_at]
                    .iter()
                    .any(|t| GermanRelativeClauseComma::is_verb_token(t, document))
            {
                continue;
            }
            // *Pläne des Magistrates der Stadt Betriebe anzusiedeln*, *der Versuch
            // der Erfolgsqualifikation abzugrenzen*: a genitive behind the noun.
            // The comma would go behind the genitive, if the group is the noun's
            // at all.
            if tokens.get(noun_at + 1).is_some_and(|after| {
                Self::word_in(after, document, &["des", "der", "eines", "einer"])
            }) {
                continue;
            }
            let noun: String = document
                .get_span_content(&tokens[noun_at].span)
                .iter()
                .collect();
            lints.push(Lint {
                span: tokens[noun_at].span,
                lint_kind: LintKind::Punctuation,
                suggestions: vec![Suggestion::InsertAfter(vec![','])],
                priority: 28,
                message: format!(
                    "Die Infinitivgruppe hängt von »{noun}« ab und wird mit Komma abgetrennt."
                ),
            });
        }
        lints
    }

    /// *am Montag den 3. Mai*: the date is an apposition to the weekday and
    /// takes a comma in front (*am Montag, dem 3. Mai* or *am Montag, den 3.
    /// Mai*).
    fn missing_comma_before_date(tokens: &[&Token], document: &Document) -> Vec<Lint> {
        const WEEKDAYS: &[&str] = &[
            "montag",
            "dienstag",
            "mittwoch",
            "donnerstag",
            "freitag",
            "samstag",
            "sonntag",
        ];
        const MONTHS: &[&str] = &[
            "januar",
            "jänner",
            "februar",
            "märz",
            "april",
            "mai",
            "juni",
            "juli",
            "august",
            "september",
            "oktober",
            "november",
            "dezember",
        ];
        let mut lints = Vec::new();
        for at in 0..tokens.len().saturating_sub(2) {
            if !Self::word_in(tokens[at], document, WEEKDAYS) {
                continue;
            }
            let with_article = Self::word_in(tokens[at + 1], document, &["den", "dem"])
                && matches!(tokens[at + 2].kind, TokenKind::Number(_));
            // *Montag 26. Mai*: the day as an ordinal and a month behind it,
            // no article. A bare number (*Montag 3 Stunden*) is no date.
            // The parser joins the point to the day ("26."); a separate point is accepted too.
            let day_with_point = matches!(tokens[at + 1].kind, TokenKind::Number(_))
                && document.get_span_content(&tokens[at + 1].span).last() == Some(&'.');
            let day_then_point = matches!(tokens[at + 1].kind, TokenKind::Number(_))
                && tokens
                    .get(at + 2)
                    .is_some_and(|t| matches!(t.kind, TokenKind::Punctuation(Punctuation::Period)));
            let without_article = if day_with_point {
                Self::word_after(tokens[at + 1], document)
                    .is_some_and(|t| Self::word_in(t, document, MONTHS))
            } else {
                day_then_point
                    && Self::word_after(tokens[at + 2], document)
                        .is_some_and(|t| Self::word_in(t, document, MONTHS))
            };
            if with_article || without_article {
                lints.push(Lint {
                    span: tokens[at].span,
                    lint_kind: LintKind::Punctuation,
                    suggestions: vec![Suggestion::InsertAfter(vec![','])],
                    priority: 28,
                    message: "Das Datum erläutert den Wochentag und wird mit Komma angeschlossen."
                        .to_owned(),
                });
            }
        }
        lints
    }

    /// *Er gab an, der NSDAP angehört zu haben und ersuchte um …*: an
    /// infinitive group opened with a comma is closed with one when the main
    /// clause carries on behind it with *und* and a finite verb of its own.
    /// *… zu essen und zu trinken* joins two infinitives and stays as it is.
    fn missing_comma_closing_infinitive_group(tokens: &[&Token], document: &Document) -> Vec<Lint> {
        let lower = |t: &Token| -> String {
            document
                .get_span_content(&t.span)
                .iter()
                .flat_map(|c| c.to_lowercase())
                .collect()
        };
        let mut lints = Vec::new();
        for at in 1..tokens.len().saturating_sub(2) {
            let verb = tokens[at];
            let ends_group = (lower(tokens[at - 1]) == "zu" && verb.kind.is_word())
                || (verb.kind.is_word() && has_zu_infix(&lower(verb)));
            if !ends_group
                || !GermanRelativeClauseComma::is_verb_token(verb, document)
                || lower(tokens[at + 1]) != "und"
            {
                continue;
            }
            let next = tokens[at + 2];
            let next_text = document.get_span_content_str(&next.span);
            // A finite verb: lower case, not an infinitive in *-n*, not *zu*.
            let finite = next.kind.is_word()
                && next_text.chars().next().is_some_and(char::is_lowercase)
                && !next_text.ends_with('n')
                && GermanRelativeClauseComma::is_verb_token(next, document)
                && !next.kind.is_noun()
                && !next.kind.is_adverb()
                // A weak preterite reads as an adjective too (*ersuchte*); as one
                // it would stand before its noun.
                && !(next.kind.is_adjective()
                    && tokens.get(at + 3).is_some_and(|t| {
                        document
                            .get_span_content(&t.span)
                            .first()
                            .is_some_and(|c| c.is_uppercase())
                    }));
            if !finite {
                continue;
            }
            // The group was opened with a comma, and the clause behind that
            // comma is the group itself, not a relative or subordinate clause.
            let Some(comma) = tokens[..at]
                .iter()
                .rposition(|t| !matches!(t.kind, TokenKind::Word(_) | TokenKind::Number(_)))
            else {
                continue;
            };
            if !matches!(
                tokens[comma].kind,
                TokenKind::Punctuation(Punctuation::Comma)
            ) {
                continue;
            }
            let group = &tokens[comma + 1..at];
            let opens_clause = group.first().is_some_and(|first| {
                let first = lower(first);
                SUBORDINATORS.contains(&first.as_str())
                    || matches!(
                        first.as_str(),
                        "der"
                            | "die"
                            | "das"
                            | "dem"
                            | "den"
                            | "deren"
                            | "dessen"
                            | "welche"
                            | "welcher"
                            | "welches"
                            | "wo"
                            | "wie"
                            | "was"
                            | "wer"
                            | "ob"
                    ) && group.len() > 1
                        && !group[1].kind.is_word()
            });
            // A relative pronoun opens a clause when a verb ends it before *zu*:
            // *…, die er versprach zu halten und …*. Any finite verb in the group
            // other than the participle before *zu haben* rules the group out.
            let finite_inside = group.iter().take(group.len().saturating_sub(2)).any(|t| {
                GermanRelativeClauseComma::is_verb_token(t, document)
                    && document
                        .get_span_content(&t.span)
                        .first()
                        .is_some_and(|c| c.is_lowercase())
                    && !t.kind.is_adjective()
            });
            if opens_clause || finite_inside || group.is_empty() {
                continue;
            }
            lints.push(Lint {
                span: verb.span,
                lint_kind: LintKind::Punctuation,
                suggestions: vec![Suggestion::InsertAfter(vec![','])],
                priority: 28,
                message:
                    "Die Infinitivgruppe endet hier und wird auch hinten mit Komma abgeschlossen."
                        .to_owned(),
            });
        }
        lints
    }

    /// *Sie ist beliebt auch wenn es keiner weiß*: *auch wenn*, *selbst wenn*
    /// and *sogar wenn* are one conjunction, and the comma goes in front of the
    /// pair. *wenn* alone is left out of [`SUBORDINATORS`] for *wenn auch*; with
    /// the particle in front it is unambiguous.
    fn missing_comma_before_concessive(tokens: &[&Token], document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();
        for at in 1..tokens.len().saturating_sub(1) {
            let previous = tokens[at - 1];
            if !Self::word_in(tokens[at], document, &["auch", "selbst", "sogar"])
                || !Self::word_in(tokens[at + 1], document, &["wenn"])
                || !matches!(previous.kind, TokenKind::Word(_))
                || Self::word_in(previous, document, CLAUSE_COORDINATORS)
                || document
                    .get_span_content(&previous.span)
                    .last()
                    .is_some_and(|c| *c == '.')
                // A heading on the line before: *Geschichte* / *Auch wenn …*.
                || document.get_source()[previous.span.end..tokens[at].span.start].contains(&'\n')
            {
                continue;
            }
            let pair: String = document.get_span_content(&tokens[at].span).iter().collect();
            lints.push(Lint {
                span: previous.span,
                lint_kind: LintKind::Punctuation,
                suggestions: vec![Suggestion::InsertAfter(vec![','])],
                priority: 28,
                message: format!(
                    "»{pair} wenn« leitet einen Nebensatz ein. Davor steht ein Komma."
                ),
            });
        }
        lints
    }

    /// *Ich gehe in die Stadt um Schuhe zu kaufen*: an infinitive group
    /// opened by *um*, *ohne*, *statt* or *anstatt* always takes a comma.
    ///
    /// The preposition *um* is far more frequent than the conjunction, so the
    /// group has to show itself: a *zu* in the same clause with a verb behind
    /// it (*zu kaufen*), or a verb with *zu* inside (*anzurufen*). And a verb
    /// has to stand in front of the group, or the group may be the sentence's
    /// subject: *Kaffee ohne Zucker zu trinken ist gesund*.
    fn missing_comma_before_infinitive_group(
        tokens: &[&Token],
        index: usize,
        document: &Document,
    ) -> Option<Lint> {
        const NOT_AFTER: &[&str] = &[
            "und", "oder", "aber", "sondern", "sowie", "nicht", "nur", "auch", "gerade", "vor",
            "allem",
        ];
        // *um* the preposition, governed by what stands before it: *es geht
        // um*, *sich kümmern um*, *der Kampf um*, *rund um*.
        const GOVERN_UM: &[&str] = &[
            "geht",
            "ging",
            "gehen",
            "handelt",
            "handelte",
            "dreht",
            "drehte",
            "steht",
            "stand",
            "kümmert",
            "kümmern",
            "kümmerte",
            "bemüht",
            "bemühen",
            "bittet",
            "bat",
            "bitten",
            "kämpft",
            "kämpfen",
            "wirbt",
            "werben",
            "ringt",
            "ringen",
            "sorgt",
            "sorgen",
            "streiten",
            "streitet",
            "beneiden",
            "beneidet",
            "schlecht",
            "gut",
            "rund",
            "sich",
            "kampf",
            "streit",
            "sorge",
            "angst",
            "bitte",
            "frage",
            "wettbewerb",
            "rennen",
            "wettlauf",
            "konkurrenz",
            "bemühungen",
            "debatte",
            "diskussion",
            "gedanken",
            "verhandlungen",
            "ringen",
        ];
        // *um bis zu 38 Prozent*, *um so mehr*, *um etwa die Hälfte*: a degree.
        const DEGREE: &[&str] = &[
            "so", "bis", "etwa", "ca", "knapp", "fast", "rund", "mehr", "weniger", "einiges",
            "vieles", "einige", "wenige", "mehrere", "viele", "halb", "viertel", "ein", "eine",
            "einen", "das", "die", "den",
        ];
        // *um halb acht Uhr*, *um einige Tage verzögern*, *um Wochen*: a time
        // or a measure in the group makes *um* the preposition.
        const MEASURES: &[&str] = &[
            "uhr",
            "tag",
            "tage",
            "tagen",
            "woche",
            "wochen",
            "monat",
            "monate",
            "monaten",
            "jahr",
            "jahre",
            "jahren",
            "stunde",
            "stunden",
            "minute",
            "minuten",
            "sekunde",
            "sekunden",
            "prozent",
            "punkt",
            "punkte",
            "euro",
            "dollar",
            "meter",
            "kilometer",
            "grad",
            "stück",
        ];
        // *Jeder hat ohne Beschränkung … das Recht zu wählen*: when the only
        // verbs in front take a *zu*-infinitive themselves, the infinitive is
        // theirs and *ohne*, *um* are prepositions.
        const ZU_GOVERNORS: &[&str] = &[
            "hat", "haben", "habe", "hast", "habt", "hatte", "hatten", "ist", "sind", "war",
            "waren", "scheint", "scheinen", "schien", "pflegt", "pflegen",
        ];
        let token = tokens[index];
        if !Self::word_in(token, document, INFINITIVE_GROUP_OPENERS) {
            return None;
        }
        let previous = tokens.get(index.checked_sub(1)?)?;
        let next = tokens.get(index + 1)?;
        let is_um = Self::word_in(token, document, &["um"]);
        // *Stufe um Stufe*, *Schritt um Schritt*.
        let repeated = next.kind.is_word()
            && document.get_span_content(&next.span) == document.get_span_content(&previous.span);
        if repeated
            || (is_um
                && (matches!(next.kind, TokenKind::Number(_))
                    || (Self::word_in(next, document, DEGREE)
                        && !Self::word_in(
                            next,
                            document,
                            &["das", "die", "den", "ein", "eine", "einen"],
                        ))))
        {
            return None;
        }
        // *an statt zu laufen* is *anstatt* written apart, which is the
        // mistake there; a comma after *an* would not fix it.
        if Self::word_in(token, document, &["statt"]) && Self::word_in(previous, document, &["an"])
        {
            return None;
        }
        if !matches!(previous.kind, TokenKind::Word(_))
            || Self::word_in(previous, document, NOT_AFTER)
            || document
                .get_span_content(&previous.span)
                .last()
                .is_some_and(|c| *c == '.')
        {
            return None;
        }
        let in_clause = |range: std::ops::Range<usize>| {
            tokens[range]
                .iter()
                .take_while(|t| !matches!(t.kind, TokenKind::Punctuation(_)))
                .copied()
                .collect::<Vec<&Token>>()
        };
        let lower = |t: &Token| -> String {
            document
                .get_span_content(&t.span)
                .iter()
                .flat_map(|c| c.to_lowercase())
                .collect()
        };
        let is_infinitive = |t: &Token| {
            let word = lower(t);
            ["en", "ern", "eln"].iter().any(|e| word.ends_with(e))
                && document
                    .get_span_content(&t.span)
                    .first()
                    .is_some_and(|c| c.is_lowercase())
                && (GermanRelativeClauseComma::is_verb_token(t, document)
                    || GermanRelativeClauseComma::infinitive_is_a_verb(&word))
        };
        let after = in_clause(index + 1..tokens.len());
        let has_group = after
            .windows(2)
            .any(|pair| lower(pair[0]) == "zu" && is_infinitive(pair[1]))
            || after
                .iter()
                .any(|t| has_zu_infix(&lower(t)) && is_infinitive(t));
        if !has_group || (is_um && after.iter().any(|t| Self::word_in(t, document, MEASURES))) {
            return None;
        }
        // A finite verb in front of the group, within the same clause — not a
        // determiner with a verb reading (*sein Geld*), and not an infinitive
        // with its *zu* (*die Firma zu betreten oder … ohne*), which would make
        // the whole clause an infinitive group the preposition sits inside.
        let clause_start = tokens[..index]
            .iter()
            .rposition(|t| matches!(t.kind, TokenKind::Punctuation(_)))
            .map_or(0, |at| at + 1);
        let before = &tokens[clause_start..index];
        if is_um && before.iter().any(|t| Self::word_in(t, document, GOVERN_UM)) {
            return None;
        }
        let verbs: Vec<&Token> = before
            .iter()
            .enumerate()
            .filter(|&(at, t)| {
                GermanRelativeClauseComma::is_verb_token(t, document)
                && !t.kind.is_determiner()
                && !t.kind.is_pronoun()
                && determiner_readings(&lower(t)).is_none()
                // *sein Geld*, *ihr Auto*: the possessive, which the table
                // leaves out because the verb is as common.
                && !(matches!(lower(t).as_str(), "sein" | "ihr")
                    && before.get(at + 1).is_some_and(|n| {
                        document
                            .get_span_content(&n.span)
                            .first()
                            .is_some_and(|c| c.is_uppercase())
                    }))
                && !(at > 0 && lower(before[at - 1]) == "zu")
                && !has_zu_infix(&lower(t))
            })
            .map(|(_, t)| *t)
            .collect();
        if verbs.is_empty()
            || verbs
                .iter()
                .all(|t| Self::word_in(t, document, ZU_GOVERNORS))
        {
            return None;
        }
        let opener: String = document.get_span_content(&token.span).iter().collect();
        Some(Lint {
            span: previous.span,
            lint_kind: LintKind::Punctuation,
            suggestions: vec![Suggestion::InsertAfter(vec![','])],
            priority: 28,
            message: format!(
                "»{opener} … zu« leitet eine Infinitivgruppe ein. Davor steht ein Komma."
            ),
        })
    }

    /// One of `nouns` in any of its forms: *Absichten*, *Möglichkeiten*,
    /// *Versuche*, *des Ziels*. The umlaut plurals are spelled out.
    fn takes_infinitive(token: &Token, document: &Document, nouns: &[&str]) -> bool {
        const UMLAUT_PLURALS: &[&str] = &[
            "pläne",
            "plänen",
            "wünsche",
            "wünschen",
            "aufträge",
            "aufträgen",
            "kräfte",
            "kräften",
            "vorschläge",
            "vorschlägen",
        ];
        let word: String = document
            .get_span_content(&token.span)
            .iter()
            .flat_map(|c| c.to_lowercase())
            .collect();
        // *Plane* and *Planen* are the tarpaulin.
        if matches!(word.as_str(), "plane" | "planen") {
            return false;
        }
        UMLAUT_PLURALS.contains(&word.as_str())
            || nouns.iter().any(|noun| {
                word.strip_prefix(noun)
                    .is_some_and(|ending| ["", "s", "es", "e", "n", "en", "er"].contains(&ending))
            })
    }

    /// The next word in the document behind `token`, across a sentence end: the
    /// full stop of an ordinal (*26. Mai*) ends the sentence for the tokenizer.
    fn word_after<'a>(token: &Token, document: &'a Document) -> Option<&'a Token> {
        document
            .get_tokens()
            .iter()
            .skip_while(|t| t.span.start <= token.span.start)
            .find(|t| !t.kind.is_whitespace())
    }

    fn word_in(token: &Token, document: &Document, set: &[&str]) -> bool {
        let word: String = document
            .get_span_content(&token.span)
            .iter()
            .flat_map(|c| c.to_lowercase())
            .collect();
        set.contains(&word.as_str())
    }
}

impl Linter for GermanSubordinateComma {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();

        for sentence in document.iter_sentences() {
            let tokens: Vec<&Token> = sentence
                .iter()
                .filter(|t| !t.kind.is_whitespace())
                .collect();

            if let Some(lint) = Self::missing_comma_after_initial_clause(&tokens, document) {
                lints.push(lint);
            }
            lints.extend(Self::missing_comma_after_initial_infinitive_group(
                &tokens, document,
            ));
            lints.extend(Self::missing_comma_after_answer_particle(&tokens, document));
            lints.extend(Self::missing_comma_before_indirect_question(
                &tokens, document,
            ));
            lints.extend(Self::missing_comma_before_reported_clause(
                &tokens, document,
            ));
            lints.extend(Self::missing_comma_before_bis_clause(&tokens, document));
            lints.extend(Self::missing_comma_before_concessive(&tokens, document));
            lints.extend(Self::missing_comma_before_date(&tokens, document));
            lints.extend(Self::missing_comma_closing_infinitive_group(
                &tokens, document,
            ));
            lints.extend(Self::missing_comma_after_noun_with_infinitive(
                &tokens, document,
            ));

            for (index, token) in tokens.iter().enumerate() {
                if let Some(lint) =
                    Self::missing_comma_before_infinitive_group(&tokens, index, document)
                {
                    lints.push(lint);
                    continue;
                }
                // *wenn* is left out of the list for *wenn auch* and the
                // comparisons *als wenn*, *wie wenn*; anywhere else it opens a
                // clause. *auch wenn* is `missing_comma_before_concessive`'s.
                let opens_wenn_clause = Self::word_in(token, document, &["wenn"])
                    && document
                        .get_span_content(&token.span)
                        .first()
                        .is_some_and(|c| c.is_lowercase())
                    // *wenn möglich*, *wenn nötig*: an elliptical phrase, whose
                    // comma is the writer's choice.
                    && !tokens.get(index + 1).is_some_and(|next| {
                        Self::word_in(
                            next,
                            document,
                            &[
                                "auch",
                                "möglich",
                                "nötig",
                                "notwendig",
                                "erforderlich",
                                "gewünscht",
                                "überhaupt",
                                "nicht",
                            ],
                        )
                    })
                    && !index.checked_sub(1).is_some_and(|i| {
                        Self::word_in(
                            tokens[i],
                            document,
                            &[
                                "als", "wie", "außer", "auch", "selbst", "sogar", "nur", "bloß",
                            ],
                        )
                    });
                if !matches!(token.kind, TokenKind::Word(_))
                    || !(Self::is_subordinator(token, document) || opens_wenn_clause)
                {
                    continue;
                }

                // Opening a sentence or a bracket, there is nothing to separate.
                let Some(previous) = index.checked_sub(1).map(|i| tokens[i]) else {
                    continue;
                };

                // Any punctuation before it already does the separating — a
                // comma, a dash, a colon, an opening bracket.
                if !matches!(previous.kind, TokenKind::Word(_)) {
                    continue;
                }

                // An abbreviation's full stop already separates the clauses:
                // "..., z. B. weil ...". The tokenizer keeps the dot on the
                // token, so the word before is "B.".
                if document
                    .get_span_content(&previous.span)
                    .last()
                    .is_some_and(|c| *c == '.')
                {
                    continue;
                }

                // *Kannst du dass bitte machen?*: the pronoun *das* misspelt,
                // with a particle of the request behind it. No clause starts
                // with *bitte*, *mal* or *doch*, so no comma either.
                if Self::word_in(token, document, &["dass"])
                    && tokens.get(index + 1).is_some_and(|next| {
                        Self::word_in(
                            next,
                            document,
                            &[
                                "bitte", "mal", "doch", "schnell", "nochmal", "gerade", "eben",
                                "kurz",
                            ],
                        )
                    })
                {
                    continue;
                }

                // The conjunction as a word, not as a conjunction.
                if Self::is_hyphenated_mention(&tokens, index)
                    || Self::is_bare_mention(&tokens, index, document)
                {
                    continue;
                }

                // Part of a fixed phrase that is not a clause.
                let conjunction_text: String =
                    document.get_span_content(&token.span).iter().collect();
                if FIXED_PHRASES.iter().any(|(first, second)| {
                    *second == conjunction_text && Self::word_in(previous, document, &[first])
                }) {
                    continue;
                }

                // The comma may belong further left, in front of the particles
                // or the coordinating conjunction that govern the clause.
                if Self::comma_is_further_left(&tokens, index, document) {
                    continue;
                }

                let conjunction: String = document.get_span_content(&token.span).iter().collect();
                if TEMPORAL_SUBORDINATORS.contains(&conjunction.as_str())
                    && Self::word_in(previous, document, TEMPORAL_MODIFIERS)
                {
                    continue;
                }

                if conjunction == "dass" && Self::word_in(previous, document, DASS_MODIFIERS) {
                    continue;
                }
                if conjunction == "ob" && Self::word_in(previous, document, OB_MODIFIERS) {
                    continue;
                }

                lints.push(Lint {
                    span: previous.span,
                    lint_kind: LintKind::Punctuation,
                    suggestions: vec![Suggestion::InsertAfter(vec![','])],
                    priority: 28,
                    message: format!(
                        "»{conjunction}« leitet einen Nebensatz ein. Davor steht ein Komma."
                    ),
                });
            }
        }

        lints
    }

    fn description(&self) -> &str {
        "Setzt das Komma vor einer unterordnenden Konjunktion (»weil«, »obwohl«, »falls«) und vor »um … zu«."
    }
}

#[cfg(test)]
mod tests {
    use super::GermanSubordinateComma;
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn lint_count(text: &str) -> usize {
        let dict = combined_german_dictionary();
        let document = Document::new(text, &PlainGerman, &dict);
        GermanSubordinateComma.lint(&document).len()
    }

    #[test]
    fn an_indirect_question_needs_a_comma() {
        for text in [
            "Sie fragte mich wann wir losfahren.",
            "Ich weiß nicht wo er wohnt.",
            "Er fragt warum das so ist.",
            "Wir wissen nicht genau wie viel es kostet.",
        ] {
            assert_eq!(lint_count(text), 1, "{text}");
        }
        for text in [
            "Sie fragte mich, wann wir losfahren.",
            "Ich weiß was.",
            "Er macht es so wie immer.",
            "Ich frage dich wie immer.",
            "Das ist der Ort, wo er wohnt.",
        ] {
            assert_eq!(lint_count(text), 0, "{text}");
        }
    }

    #[test]
    fn an_infinitive_group_after_its_noun_needs_a_comma() {
        for text in [
            "Ziel der Regelung ist das Ziel diese Sollwerte zu erreichen.",
            "Er hatte die Absicht nach Berlin zu fahren.",
            "Wir haben die Möglichkeit das Haus günstig zu kaufen.",
            "Trump dementierte Absichten den Iran nach den Zwischenwahlen anzugreifen.",
            "Es gab viele Möglichkeiten das Problem zu lösen.",
            "Er gab an, der NSDAP angehört zu haben und ersuchte gleichzeitig um Aufnahme.",
            "Raja nutzt die Chance, sich an Renu zu rächen und gibt bei der Polizei an.",
        ] {
            assert_eq!(lint_count(text), 1, "{text}");
        }
        for text in [
            "Er hatte die Absicht, nach Berlin zu fahren.",
            "Ich habe keine Lust zu arbeiten.",
            "Das Ziel ist schwer zu erreichen.",
            "Sie gab ihm die Möglichkeit und versuchte zu helfen.",
            "Der Plan war gut zu verstehen.",
            "Sie trafen sich, um über die Möglichkeit einer Synthese zu diskutieren.",
            "Das heißt, aus einer Absicht heraus eine Lehre zu vermitteln.",
            "Es ist möglich, ohne Gefahr eines Irrtums zu entscheiden.",
            "Kannst du dass bitte machen?",
            "Trump dementierte Absichten, den Iran anzugreifen.",
            "Er versuchte, etwas zu essen und zu trinken.",
            "Er versuchte, etwas zu essen und trinken.",
            "Er gab an, der NSDAP angehört zu haben, und ersuchte um Aufnahme.",
            "Er bat sie zu kommen und ging.",
            "Die Planen zu öffnen dauert.",
            "So ist es möglich, die Entscheidung für die Schreibweise nachzuvollziehen.",
            "Der Versuch ist streng von dem Versuch der Erfolgsqualifikation abzugrenzen.",
            "Sie bieten die Möglichkeit, auf den Weg zu wechseln und umgekehrt.",
        ] {
            assert_eq!(lint_count(text), 0, "{text}");
        }
    }

    #[test]
    fn a_wenn_clause_needs_a_comma() {
        for text in [
            "Ich bin erreichbar, außer zwischen 10 und 11 Uhr wenn wir beim Arzt sind.",
            "Es wäre gut wenn du dich meldest.",
        ] {
            assert_eq!(lint_count(text), 1, "{text}");
        }
        for text in [
            "Es wäre gut, wenn du dich meldest.",
            "Er tut so, als wenn nichts wäre.",
            "Wenn auch nur kurz, er kam.",
            "Ich komme nur wenn du kommst.",
            "Vierteilige werden wenn möglich vermieden.",
        ] {
            assert_eq!(lint_count(text), 0, "{text}");
        }
    }

    #[test]
    fn an_opening_clause_group_or_particle_needs_a_comma() {
        for text in [
            "Wo du bist ist mir egal.",
            "Was du sagst stimmt nicht.",
            "Statt zu arbeiten schläft er.",
            "Um zu gewinnen muss man trainieren.",
            "Ja ich komme gerne.",
            "Nein das geht nicht.",
            "Ach so das wusste ich nicht.",
        ] {
            assert_eq!(lint_count(text), 1, "{text}");
        }
        for text in [
            "Wo bist du?",
            "Was du sagst?",
            "Wie lange dauert das?",
            "Ja doch.",
            "Ja gerne komme ich.",
            "Nein danke.",
            "Um zu gewinnen, muss man trainieren.",
            "Statt zu arbeiten und zu lernen, schläft er.",
            "Ja, ich komme.",
            "Wo du bist, ist mir egal.",
        ] {
            assert_eq!(lint_count(text), 0, "{text}");
        }
    }

    #[test]
    fn a_date_after_the_weekday_needs_a_comma() {
        assert_eq!(
            lint_count("Das Treffen findet am Montag den 3. Mai statt."),
            1
        );
        assert_eq!(
            lint_count("Das Treffen findet am Montag, dem 3. Mai, statt."),
            0
        );
        assert_eq!(
            lint_count("Selbstverlag, Starnberg, Montag 26. Mai 2025."),
            1
        );
        for text in [
            "Selbstverlag, Starnberg, Montag, 26. Mai 2025.",
            "Er arbeitete am Montag 3 Stunden.",
            "Am Montag 3. Platz, am Dienstag Sieg.",
        ] {
            assert_eq!(lint_count(text), 0, "{text}");
        }
    }

    #[test]
    fn a_concessive_pair_needs_a_comma() {
        assert_eq!(
            lint_count("Die Torte ist beliebt auch wenn keiner sie kennt."),
            1
        );
        assert_eq!(lint_count("Er kommt selbst wenn es regnet."), 1);
        for text in [
            "Die Torte ist beliebt, auch wenn keiner sie kennt.",
            "Auch wenn es regnet, kommt er.",
            "Er kommt und auch wenn es regnet, bleibt er.",
            "Wenn auch nur kurz, er kam.",
            "Geschichte\nAuch wenn der Begriff alt ist, gilt er.",
        ] {
            assert_eq!(lint_count(text), 0, "{text}");
        }
    }

    #[test]
    fn a_bis_clause_needs_a_comma() {
        assert_eq!(
            lint_count("Die Kinder haben gespielt bis es dunkel wurde."),
            1
        );
        assert_eq!(
            lint_count("Die Kinder haben gespielt, bis es dunkel wurde."),
            0
        );
        assert_eq!(lint_count("Wir haben gearbeitet bis morgen."), 0);
        assert_eq!(lint_count("Wir haben gewartet bis zum Abend."), 0);
    }

    #[test]
    fn a_reported_clause_needs_a_comma() {
        for text in [
            "Er hat gesagt er kommt später.",
            "Ich glaube er hat recht.",
            "Ich hoffe es geht dir gut.",
            "Sie sagte sie sei müde.",
        ] {
            assert_eq!(lint_count(text), 1, "{text}");
        }
        for text in [
            "Er hat gesagt, er kommt später.",
            "Er sagte es ihm.",
            "Ich glaube ihr kein Wort.",
            "Er meint es ernst.",
            "Ich glaube es dir.",
            "Er hat gesagt, dass er kommt.",
        ] {
            assert_eq!(lint_count(text), 0, "{text}");
        }
    }

    #[test]
    fn a_sentence_initial_clause_needs_its_closing_comma() {
        assert_eq!(lint_count("Wenn du kommst bringe Brot mit."), 1);
        assert_eq!(lint_count("Als ich nach Hause kam war niemand da."), 1);
        assert_eq!(lint_count("Weil es regnet bleiben wir zu Hause."), 1);
        assert_eq!(lint_count("Dass er kommt freut mich."), 1);
        assert_eq!(lint_count("Wenn du heute kommst bringe Brot mit."), 1);
        assert_eq!(lint_count("Wenn es heute regnet bleiben wir zu Hause."), 1);
        // *heute* is not the preterite of *heuen* here: the clause has no
        // closing verb, it is a main clause with a misspelled article.
        assert_eq!(lint_count("Dass Wetter ist heute schlecht."), 0);
        assert_eq!(lint_count("Wenn du kommst, bringe Brot mit."), 0);
        assert_eq!(lint_count("Wenn du kommen willst, bringe Brot mit."), 0);
        assert_eq!(lint_count("Als Kind spielte ich oft draußen."), 0);
        assert_eq!(lint_count("Ob er kommt."), 0);
        assert_eq!(
            lint_count("Dass essen gehen ein Luxus sein soll, bezweifle ich."),
            0
        );
        assert_eq!(
            lint_count("Wenn du Schluss machen willst bevor es angefangen hat, dann leg los."),
            1
        );
    }

    #[test]
    fn requires_the_comma() {
        for text in [
            "Er blieb zu Hause weil er krank war.",
            "Das Haus steht noch obwohl es alt ist.",
            "Wir gehen los sobald der Regen aufhört.",
            "Sie rief an nachdem sie angekommen war.",
        ] {
            assert_eq!(lint_count(text), 1, "should fire on {text:?}");
        }
    }

    #[test]
    fn accepts_the_comma() {
        for text in [
            "Er blieb zu Hause, weil er krank war.",
            "Das Haus steht noch, obwohl es alt ist.",
            "Weil er krank war, blieb er zu Hause.",
            "Er kam (weil es regnete) mit dem Auto.",
            "Er kam, vor allem weil es regnete.",
            "Er blieb zu Hause, und weil er krank war, rief er an.",
            // A temporal modifier fuses with a temporal conjunction.
            "Noch bevor es zum Sturm kam, war alles fertig.",
            "Das Flugzeug verunglückte, kurz nachdem es gestartet war.",
            // "je nachdem" is a fixed phrase, not a clause.
            "Die Städte haben je nachdem einen oder zwei Bürgermeister.",
            // An abbreviation's own full stop already separates them.
            "Angaben sind schwierig, z. B. weil sie eine Unterscheidung treffen.",
            // Capitalized, it is a name: "im Kabinett Weil III".
            "Er war Minister im Kabinett Weil III und später Bevollmächtigter.",
        ] {
            assert_eq!(lint_count(text), 0, "should not fire on {text:?}");
        }
    }

    /// German puts a modal particle or a focus adverb between the comma and the
    /// conjunction, and the comma is then correct where it stands. Every one of
    /// these came out of a corpus of published prose, where looking exactly one
    /// token to the left reported them as missing commas.
    #[test]
    fn a_particle_may_stand_between_the_comma_and_the_conjunction() {
        for text in [
            "Im Lexikon fehlt der Eintrag, wohl weil man sich sonst verlieren würde.",
            "Er stieg auf den Berg, einfach weil der Berg da ist.",
            "Die Planung wurde verschoben, vermutlich weil die Kosten stiegen.",
            "Etwas anderes gilt nur dann, etwa weil er davon wusste.",
            "Sie führte Gespräche, ganz einfach weil es ihr Spaß machte.",
        ] {
            assert_eq!(lint_count(text), 0, "should not fire on {text:?}");
        }
    }

    /// A noun or a verb in between is a clause of its own, and the comma really
    /// is missing.
    #[test]
    fn a_noun_between_them_does_not_license_the_comma() {
        for text in [
            "Die Nanostruktur ist von Bedeutung weil große Oberflächen entstehen.",
            "Er ging nach Hause und blieb dort weil er krank war.",
        ] {
            assert_eq!(lint_count(text), 1, "should fire on {text:?}");
        }
    }

    /// Talking about the conjunction rather than using it. German writes that as
    /// a hyphenated compound, and a linguistics article is full of them.
    #[test]
    fn a_hyphenated_mention_is_not_a_clause() {
        for text in [
            "Die Ersetzung erweist den obwohl-Satz als Satzglied.",
            "Er untersucht die weil-Konstruktion im Neuhochdeutschen.",
        ] {
            assert_eq!(lint_count(text), 0, "should not fire on {text:?}");
        }
    }

    /// `dass` is the most common missing comma in written German.
    #[test]
    fn requires_the_comma_before_dass() {
        for text in [
            "Ich weiß dass du recht hast.",
            "Wir hoffen dass alles gut geht.",
            "Er glaubt dass er alles versteht.",
            "Sie hat gesagt dass sie kommt.",
            "Es ist bekannt dass Wasser bei 100 Grad siedet.",
            "Mit dem Ergebnis dass zunehmend Söldner auftraten.",
        ] {
            assert_eq!(lint_count(text), 1, "should fire on {text:?}");
        }
    }

    #[test]
    fn accepts_the_comma_before_dass() {
        for text in [
            "Ich weiß, dass du recht hast.",
            "Wir hoffen, dass alles gut geht.",
            "Dass er kam, war überraschend.",
            "Er sagte, und dass ist wichtig, nichts dazu.",
        ] {
            assert_eq!(lint_count(text), 0, "should stay quiet on {text:?}");
        }
    }

    /// The comma goes in front of the pair, not between its halves.
    #[test]
    fn a_two_part_conjunction_keeps_its_comma_on_the_left() {
        for text in [
            "Er ging ohne dass jemand es merkte.",
            "Sie half statt dass sie zusah.",
            "Er lief so dass er rechtzeitig ankam.",
            "Das ist zu teuer als dass wir es kaufen.",
            "Kaum dass er saß, klingelte es.",
        ] {
            assert_eq!(lint_count(text), 0, "should stay quiet on {text:?}");
        }
    }

    /// A grammar article names its conjunctions instead of using them.
    #[test]
    fn a_named_conjunction_is_not_a_clause() {
        for text in [
            "Subjunktionen sind vor allem dass und ob.",
            "Inhaltssätze mit dass oder ob sind häufig.",
            "Häufige Konjunktionen sind dabei weil, da und zumal.",
            "Die Subjunktionen weil und da werden gleich verwendet.",
            "Hingegen können dass und ob nur mit finiten Nebensätzen stehen.",
        ] {
            assert_eq!(lint_count(text), 0, "should stay quiet on {text:?}");
        }
    }

    /// Two conjunctions in a row is a mention, not a clause.
    #[test]
    fn a_conjunction_behind_a_conjunction_is_a_mention() {
        assert_eq!(
            lint_count("Es markiert, wogegen dass vor allem Aussagen markiert."),
            0
        );
    }

    /// The additions must not silence what the rule already caught.
    #[test]
    fn the_older_conjunctions_still_fire() {
        for text in [
            "Er blieb zu Hause weil er krank war.",
            "Wir gehen los sobald der Regen aufhört.",
            "Sie ging nach Hause weil es regnete.",
        ] {
            assert_eq!(lint_count(text), 1, "should fire on {text:?}");
        }
    }

    #[test]
    fn an_infinitive_group_with_um_takes_a_comma() {
        for text in [
            "Ich gehe morgen in die Stadt um Schuhe zu kaufen.",
            "Er kam um uns zu helfen.",
            "Sie ging ohne sich zu verabschieden.",
            "Er spielte statt zu lernen.",
            "Ich rufe dich an um dich einzuladen.",
            "Wir fahren nach Berlin um unsere Oma zu besuchen.",
            "Sie hat lange gearbeitet um Geld zu verdienen.",
        ] {
            assert_eq!(lint_count(text), 1, "should fire on {text:?}");
        }
    }

    #[test]
    fn um_as_a_preposition_or_with_its_comma_is_quiet() {
        for text in [
            "Ich gehe morgen in die Stadt, um Schuhe zu kaufen.",
            "Wir treffen uns um 12 Uhr.",
            "Er fuhr um die Ecke zu schnell.",
            "Kaffee ohne Zucker zu trinken ist gesund.",
            "Um zu gewinnen, muss man trainieren.",
            "Er bat um Hilfe.",
            "Es geht um zu viel Geld.",
            "Er fuhr mit dem Rad an statt zu laufen.",
            "Er kam nicht um zu helfen, sondern um zu stören.",
            "Er kam und um zu helfen blieb er.",
            "Sie ging ohne Schirm zu ihrer Freundin.",
            "Es scheint schlecht um das gute alte Vinyl zu stehen.",
            "Wer sich zutraut, sein Geld ohne Beratung anzulegen, kann sparen.",
            "Er lernt, Codesequenzen Stufe um Stufe zu optimieren.",
            "Die Chips sind nun um bis zu 38 Prozent günstiger zu haben.",
            "Anwender brauchen sich um die Kompression keine Gedanken zu machen.",
            "Es ist möglich, Arbeitsplätze rund um das Web zu schaffen.",
            "Doch ist diese Beschränktheit um so weniger zu tadeln.",
            "Es ist verboten, die Firma ohne Genehmigung zu betreten oder Geräte ohne Genehmigung zu benutzen.",
            "Jeden Tag gehe ich um halb acht aus dem Haus und fange um zehn Uhr zu arbeiten an.",
            "Jeder hat ohne Beschränkung auf Rasse oder Geschlecht das Recht zu wählen.",
            "Der Schritt drohe die Auslieferung um einige Tage zu verzögern.",
            "Er drohte die Auslieferung um Wochen zu verzögern.",
        ] {
            assert_eq!(lint_count(text), 0, "should not fire on {text:?}");
        }
    }
}
