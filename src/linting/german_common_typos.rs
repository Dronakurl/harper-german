use harper_core::linting::{Lint, LintKind, Linter, Suggestion};
use harper_core::spell::Dictionary;
use harper_core::{Document, TokenStringExt};

use crate::spell::curated_german_dictionary;

/// Misspellings that Harper's German spell check cannot see, and their
/// corrections.
///
/// The keys are lower case and are looked up case-insensitively; the values
/// carry the spelling a capitalized occurrence should get. Sorted, so a new
/// entry goes where it belongs and `binary_search_by_key` can find it.
const COMMON_TYPOS: &[(&str, &str)] = &[
    ("addressiert", "adressiert"),
    ("adminstration", "Administration"),
    ("algorhitmus", "Algorithmus"),
    ("alstadt", "Altstadt"),
    ("amtsitz", "Amtssitz"),
    ("anderere", "andere"),
    ("andereren", "anderen"),
    ("andererer", "anderer"),
    ("aufenhalt", "Aufenthalt"),
    ("aufname", "Aufnahme"),
    ("auforderung", "Aufforderung"),
    ("aufsteig", "Aufstieg"),
    ("augenlied", "Augenlid"),
    ("auserdem", "außerdem"),
    ("ausgangpunkt", "Ausgangspunkt"),
    ("ausicht", "Aussicht"),
    ("ausprache", "Aussprache"),
    ("auspruch", "Ausspruch"),
    ("aussschließlich", "ausschließlich"),
    ("austellung", "Ausstellung"),
    ("austellungen", "Ausstellungen"),
    ("austerben", "Aussterben"),
    ("bauerhöfe", "Bauernhöfe"),
    ("bedürfniss", "Bedürfnis"),
    ("beindrucken", "beeindrucken"),
    ("beispielswiese", "beispielsweise"),
    ("beispielweise", "beispielsweise"),
    ("beitag", "Beitrag"),
    ("bespiel", "Beispiel"),
    ("bischofsitz", "Bischofssitz"),
    ("bundestraße", "Bundesstraße"),
    ("danaben", "daneben"),
    ("daüber", "darüber"),
    ("dollmetscher", "Dolmetscher"),
    ("durchschlagkraft", "Durchschlagskraft"),
    ("durschnitt", "Durchschnitt"),
    ("durschnittlich", "durchschnittlich"),
    ("durschnittliche", "durchschnittliche"),
    ("durschnittlichen", "durchschnittlichen"),
    ("ebenfall", "ebenfalls"),
    ("eingen", "einigen"),
    ("eingentlich", "eigentlich"),
    ("eingesetz", "eingesetzt"),
    ("eingesetzen", "eingesetzten"),
    ("engeneering", "Engineering"),
    ("entstandende", "entstandene"),
    ("erfolgslos", "erfolglos"),
    ("erstaustrahlung", "Erstausstrahlung"),
    ("erwachsenalter", "Erwachsenenalter"),
    ("festellen", "feststellen"),
    ("freimauerei", "Freimaurerei"),
    ("friedenschluss", "Friedensschluss"),
    ("fußballstadium", "Fußballstadion"),
    ("garnision", "Garnison"),
    ("gebähren", "gebären"),
    ("gegebenfalls", "gegebenenfalls"),
    ("gegnüber", "gegenüber"),
    ("gemeinsammen", "gemeinsamen"),
    ("gesetztlich", "gesetzlich"),
    ("gesichtpunkt", "Gesichtspunkt"),
    ("handelschiff", "Handelsschiff"),
    ("handies", "Handys"),
    ("herrausragend", "herausragend"),
    ("herschaft", "Herrschaft"),
    ("herscher", "Herrscher"),
    ("hervorragenste", "hervorragendste"),
    ("hingegegen", "hingegen"),
    ("hintegrund", "Hintergrund"),
    ("hochaus", "Hochhaus"),
    ("hochäuser", "Hochhäuser"),
    ("imbus", "Inbus"),
    ("inhaltstoff", "Inhaltsstoff"),
    ("intergration", "Integration"),
    ("inverstor", "Investor"),
    ("jahrhudert", "Jahrhundert"),
    ("jahundert", "Jahrhundert"),
    ("jahunderte", "Jahrhunderte"),
    ("jahunderts", "Jahrhunderts"),
    ("janur", "Januar"),
    ("kenntniss", "Kenntnis"),
    ("krankenaus", "Krankenhaus"),
    ("kriegschiff", "Kriegsschiff"),
    ("kunstoff", "Kunststoff"),
    ("landesaustellung", "Landesausstellung"),
    ("landesprache", "Landessprache"),
    ("landsmannschaftschaftliche", "landsmannschaftliche"),
    ("landwirschaft", "Landwirtschaft"),
    ("landwirschaftlich", "landwirtschaftlich"),
    ("langläufig", "landläufig"),
    ("mannschaf", "Mannschaft"),
    ("manschaft", "Mannschaft"),
    ("meerespiegel", "Meeresspiegel"),
    ("meeresspegel", "Meeresspiegel"),
    ("mehrtätige", "mehrtägige"),
    ("meistbesuchtesten", "meistbesuchten"),
    ("metereologe", "Meteorologe"),
    ("metereologisch", "meteorologisch"),
    ("mettal", "Metall"),
    ("mittelaterlich", "mittelalterlich"),
    ("mittlerweilen", "mittlerweile"),
    ("mobilar", "Mobiliar"),
    ("nahaufname", "Nahaufnahme"),
    ("namenlich", "namentlich"),
    ("namenslos", "namenlos"),
    ("neugründeten", "neugegründeten"),
    ("nordlich", "nördlich"),
    ("organistion", "Organisation"),
    ("ortteil", "Ortsteil"),
    ("rechtspruch", "Rechtsspruch"),
    ("rechtstaat", "Rechtsstaat"),
    ("rechtstaatlich", "rechtsstaatlich"),
    ("rechtwinklich", "rechtwinklig"),
    ("rechzeitig", "rechtzeitig"),
    ("resourcen", "Ressourcen"),
    ("russsische", "russische"),
    ("russsischen", "russischen"),
    ("rückrad", "Rückgrat"),
    ("rückrat", "Rückgrat"),
    ("schafte", "schaffte"),
    ("schwerpunktsmäßig", "schwerpunktmäßig"),
    ("seeman", "Seemann"),
    ("sehenwürdigkeit", "Sehenswürdigkeit"),
    ("sogenante", "sogenannte"),
    ("sogenanten", "sogenannten"),
    ("standarts", "Standards"),
    ("stehgreif", "Stegreif"),
    ("tolleranz", "Toleranz"),
    ("totkrank", "todkrank"),
    ("trotzdessen", "trotzdem"),
    ("turist", "Tourist"),
    ("umgangsprachlich", "umgangssprachlich"),
    ("umgangsprachliche", "umgangssprachliche"),
    ("umgangsprachlicher", "umgangssprachlicher"),
    ("vergleichweise", "vergleichsweise"),
    ("verwaltungsitz", "Verwaltungssitz"),
    ("vieleicht", "vielleicht"),
    ("vorausichtlich", "voraussichtlich"),
    ("vorgesetze", "Vorgesetzte"),
    ("vormachtsstellung", "Vormachtstellung"),
    ("wachholder", "Wacholder"),
    ("warscheinlich", "wahrscheinlich"),
    ("wehrmutstropfen", "Wermutstropfen"),
    ("weiterere", "weitere"),
    ("weitereren", "weiteren"),
    ("weitesgehend", "weitestgehend"),
    ("weißmachen", "weismachen"),
    ("wesendlich", "wesentlich"),
    ("widerstandkämpfer", "Widerstandskämpfer"),
    ("wiedersacher", "Widersacher"),
    ("wiedersprüche", "Widersprüche"),
    ("wirtschaflich", "wirtschaftlich"),
    ("wissenschaflich", "wissenschaftlich"),
    ("währendessen", "währenddessen"),
    ("wärend", "während"),
    ("überlicherweise", "üblicherweise"),
];

/// Misspellings that do not come from the source list below, written down by
/// hand from everyday writing. The ones with a space in the correction are
/// adverbs written closed up: *schon mal* and *nicht mal* are two words, while
/// *nochmal* is a word of its own and stays out. Same order rule as
/// [`COMMON_TYPOS`].
const HAND_PICKED_TYPOS: &[(&str, &str)] = &[
    ("bischen", "bisschen"),
    ("nichtmal", "nicht mal"),
    ("nocheinmal", "noch einmal"),
    ("packet", "Paket"),
    ("schoneinmal", "schon einmal"),
    ("schonmal", "schon mal"),
    ("wiedereinmal", "wieder einmal"),
    ("wiedermal", "wieder mal"),
];

/// Corrects the misspellings people actually make, which the spell checker
/// lets through.
///
/// German compounds freely, so Harper's spell checker accepts any word it can
/// cut into dictionary pieces. That is what makes German spell checking work
/// at all, and it is also why the most ordinary German typos sail past it:
/// *Amtsitz* cuts into `Amt` + `Sitz`, *vieleicht* into `viel` + `eicht`,
/// *Landesprache* into `Landes` + `Sprache`. Every one of them is a legal cut
/// of illegal German.
///
/// The table cannot be derived, so it is sourced and filtered instead. Source:
/// `Wikipedia:Liste von Tippfehlern`, the list de-wiki's own correction bots
/// run on, which is the same shape as Microsoft Word's autocorrect table.
/// Rebuild it by taking the entries that are a single word on both sides and
/// keeping the ones where:
///
/// - Harper reports nothing, so the entry adds something; and
/// - **both** LanguageTool and `hunspell -d de_DE` reject the misspelling.
///
/// The second condition is the one that matters. Both of those also split
/// compounds, so a word all three accept is a real German word — that is how
/// `anderseits`, `wohlgesonnen` and `Nachkommens`, which the source list calls
/// errors, stayed out. Proper names are out of scope and are dropped by hand.
///
/// Case is the writer's: the suggestion copies the casing of what was typed,
/// so `austellung` becomes `ausstellung` and is left to
/// [`super::GermanNounCapitalization`] to capitalize. Fixing both here would
/// make one lint out of two independent mistakes.
#[derive(Default)]
pub struct GermanCommonTypos;

impl GermanCommonTypos {
    fn correction(word: &[char]) -> Option<&'static str> {
        let lower: String = word.iter().flat_map(|c| c.to_lowercase()).collect();

        [COMMON_TYPOS, HAND_PICKED_TYPOS]
            .into_iter()
            .find_map(|table| {
                table
                    .binary_search_by_key(&lower.as_str(), |(wrong, _)| wrong)
                    .ok()
                    .map(|index| table[index].1)
            })
    }
}

impl GermanCommonTypos {
    /// The correction of a compound that *opens* with a misspelling from the
    /// table: *Kunstoffeimer* is *Kunststoff* + *Eimer*, and the compound
    /// splitter accepts it as *Kunst* + *off* + *eimer*, exactly as it accepts
    /// *Kunstoff* on its own.
    ///
    /// Only entries of five letters or more open a compound here, and the rest
    /// has to be a dictionary word of at least three letters by itself, written
    /// small as the compound writes it or capitalized as the noun is. Five
    /// letters keep short entries like *eingen* from matching the start of a
    /// word that merely begins the same way. Linking elements are not tried: an
    /// entry is a whole word, and a writer who knows the seam of *Kunststoffs*
    /// knows the word.
    fn compound_correction(word: &[char]) -> Option<String> {
        let lower: String = word.iter().flat_map(|c| c.to_lowercase()).collect();
        let dictionary = curated_german_dictionary();
        [COMMON_TYPOS, HAND_PICKED_TYPOS]
            .into_iter()
            .flatten()
            .filter(|(wrong, right)| {
                wrong.chars().count() >= 5
                    && !right.contains(' ')
                    && lower.starts_with(wrong)
                    && lower.chars().count() >= wrong.chars().count() + 3
            })
            .max_by_key(|(wrong, _)| wrong.len())
            // The entry may merely be how a correct compound begins: *Kenntnis* +
            // *Stand* starts with *kenntniss*, *hoch* + *ausgebildeten* with
            // *hochaus*. When the word splits into two dictionary words at a seam
            // inside the entry, it is that compound and not the typo.
            .filter(|(wrong, _)| !Self::splits_inside(&lower, wrong.chars().count()))
            .and_then(|(wrong, right)| {
                let rest: Vec<char> = lower[wrong.len()..].chars().collect();
                let mut capitalized = rest.clone();
                capitalized[0] = capitalized[0].to_uppercase().next()?;
                let is_word = |w: &[char]| {
                    dictionary.contains_exact_word(w)
                        && dictionary
                            .get_word_metadata(w)
                            .is_some_and(|m| !m.is_proper_noun())
                };
                (is_word(&rest) || is_word(&capitalized)).then(|| {
                    let rest: String = rest.into_iter().collect();
                    format!("{}{rest}", right.to_lowercase())
                })
            })
    }
}

impl GermanCommonTypos {
    /// Does `word` split into two dictionary words, with or without a linking
    /// element, at a seam before character `limit`?
    fn splits_inside(word: &str, limit: usize) -> bool {
        let dictionary = curated_german_dictionary();
        let chars: Vec<char> = word.chars().collect();
        let is_word = |part: &[char]| {
            let mut capitalized = part.to_vec();
            if let Some(first) = capitalized.first_mut() {
                *first = first.to_uppercase().next().unwrap_or(*first);
            }
            part.len() >= 3
                && (dictionary.contains_exact_word(part)
                    || dictionary.contains_exact_word(&capitalized))
        };
        (3..limit.min(chars.len())).any(|seam| {
            let (head, rest) = chars.split_at(seam);
            is_word(head)
                && ["", "s", "n", "en", "es", "er", "e"].iter().any(|link| {
                    let link: Vec<char> = link.chars().collect();
                    rest.strip_prefix(link.as_slice()).is_some_and(is_word)
                })
        })
    }
}

impl Linter for GermanCommonTypos {
    fn lint(&mut self, document: &Document) -> Vec<Lint> {
        let mut lints = Vec::new();

        for word in document.iter_words() {
            let chars = document.get_span_content(&word.span);
            let (correction, in_compound) = match Self::correction(chars) {
                Some(correction) => (correction.to_string(), false),
                None => match Self::compound_correction(chars) {
                    Some(correction) => (correction, true),
                    None => continue,
                },
            };

            let original: String = chars.iter().collect();
            let message = if in_compound {
                format!("»{original}« beginnt mit einem häufigen Tippfehler.")
            } else if correction.contains(' ') {
                format!("»{correction}« schreibt man getrennt.")
            } else {
                format!("»{original}« ist ein häufiger Tippfehler.")
            };
            lints.push(Lint {
                span: word.span,
                lint_kind: LintKind::Spelling,
                suggestions: vec![Suggestion::replace_with_match_case_str(&correction, chars)],
                message,
                priority: 19,
            });
        }

        lints
    }

    fn description(&self) -> &str {
        "Korrigiert häufige deutsche Tippfehler, die der Rechtschreibprüfung entgehen."
    }
}

#[cfg(test)]
mod tests {
    use super::{COMMON_TYPOS, GermanCommonTypos, HAND_PICKED_TYPOS};
    use crate::parsers::PlainGerman;
    use crate::spell::combined_german_dictionary;
    use harper_core::Document;
    use harper_core::linting::Linter;

    fn document(text: &str) -> Document {
        Document::new(text, &PlainGerman, &combined_german_dictionary())
    }

    fn flagged(text: &str) -> Vec<String> {
        let document = document(text);
        GermanCommonTypos
            .lint(&document)
            .into_iter()
            .map(|lint| document.get_span_content_str(&lint.span))
            .collect()
    }

    fn correction(word: &str) -> Option<&'static str> {
        let chars: Vec<char> = word.chars().collect();
        GermanCommonTypos::correction(&chars)
    }

    /// `binary_search_by_key` compares `&str` by UTF-8 bytes. A table sorted
    /// any other way — by code point after NFD, or by a locale that files `ü`
    /// under `u` — still looks orderly and silently stops finding its umlaut
    /// entries.
    #[test]
    fn the_table_is_in_the_order_the_search_assumes() {
        for pair in COMMON_TYPOS.windows(2).chain(HAND_PICKED_TYPOS.windows(2)) {
            assert!(
                pair[0].0 < pair[1].0,
                "{:?} must sort before {:?}",
                pair[0].0,
                pair[1].0
            );
        }
    }

    /// A correction that is itself a key would leave the writer fixing the
    /// same word forever.
    #[test]
    fn no_correction_is_itself_a_typo() {
        for (wrong, right) in COMMON_TYPOS.iter().chain(HAND_PICKED_TYPOS) {
            assert_eq!(correction(right), None, "{wrong} -> {right} loops");
            assert!(
                !COMMON_TYPOS.iter().any(|(other, _)| other == wrong)
                    || !HAND_PICKED_TYPOS.iter().any(|(other, _)| other == wrong),
                "{wrong} is in both tables"
            );
        }
    }

    #[test]
    fn catches_the_typos_the_compound_splitter_waves_through() {
        for (wrong, right) in [
            ("Amtsitz", "Amtssitz"),
            ("Bundestraße", "Bundesstraße"),
            ("Landesprache", "Landessprache"),
            ("vieleicht", "vielleicht"),
            ("wärend", "während"),
            ("Manschaft", "Mannschaft"),
            ("warscheinlich", "wahrscheinlich"),
            ("Kunstoff", "Kunststoff"),
        ] {
            assert_eq!(correction(wrong), Some(right), "{wrong}");
            assert_eq!(
                flagged(&format!("Hier steht {wrong} im Satz.")),
                vec![wrong]
            );
        }
    }

    #[test]
    fn catches_a_typo_at_the_start_of_a_compound() {
        for (text, right) in [
            ("Das ist ein Kunstoffeimer.", "Kunststoffeimer"),
            ("Die Kunstoffmodelle sind leicht.", "Kunststoffmodelle"),
        ] {
            let document = document(text);
            let lints = GermanCommonTypos.lint(&document);
            assert_eq!(lints.len(), 1, "{text}");
            assert!(
                format!("{:?}", lints[0].suggestions).contains(right),
                "{text}: {:?}",
                lints[0].suggestions
            );
        }
        for text in [
            "Das ist ein Kunststoffeimer.",
            "Die Kunst offenbart sich.",
            "Nach heutigem Kenntnisstand ist das so.",
            "Sie sind hochausgebildete Fachkräfte.",
            "Reize gehen auf die aufsteigende Bahn.",
        ] {
            assert!(flagged(text).is_empty(), "{text}");
        }
    }

    #[test]
    fn splits_adverbs_written_closed_up() {
        for (text, wrong, right) in [
            ("Ich habe es dir schonmal gesagt.", "schonmal", "schon mal"),
            ("Das ist nichtmal schlimm.", "nichtmal", "nicht mal"),
            ("Er hat das wiedermal vergessen.", "wiedermal", "wieder mal"),
            (
                "Ich bin wiedereinmal zu spät.",
                "wiedereinmal",
                "wieder einmal",
            ),
            ("Schonmal davon gehört?", "Schonmal", "Schon mal"),
            ("Das ist ein bischen viel.", "bischen", "bisschen"),
            ("Wir haben das Packet bekommen.", "Packet", "Paket"),
        ] {
            let document = document(text);
            let lints = GermanCommonTypos.lint(&document);
            assert_eq!(lints.len(), 1, "{text}");
            assert_eq!(document.get_span_content_str(&lints[0].span), wrong);
            assert!(
                format!("{:?}", lints[0].suggestions).contains(right),
                "{text}: {:?}",
                lints[0].suggestions
            );
        }
        for text in [
            "Sag es noch mal.",
            "Sag es nochmal.",
            "Ich war schon mal da.",
        ] {
            assert!(flagged(text).is_empty(), "{text}");
        }
    }

    /// The suggestion takes the writer's casing, not the table's. Capitalizing
    /// a lower-case noun is a second, separate mistake and belongs to
    /// `GermanNounCapitalization`.
    #[test]
    fn the_suggestion_follows_the_writers_case() {
        let document = document("Die austellung und die Austellung.");
        let lints = GermanCommonTypos.lint(&document);

        let suggestions: Vec<String> = lints
            .iter()
            .flat_map(|lint| &lint.suggestions)
            .map(|suggestion| format!("{suggestion:?}"))
            .collect();

        assert_eq!(lints.len(), 2);
        assert!(suggestions[0].contains("ausstellung"), "{suggestions:?}");
        assert!(suggestions[1].contains("Ausstellung"), "{suggestions:?}");
    }

    /// Every one of these is a word the source list calls an error and two
    /// independent spell checkers call German. They stay out of the table.
    #[test]
    fn leaves_real_german_words_alone() {
        for word in [
            "anderseits",
            "wohlgesonnen",
            "Nachkommens",
            "Autorenschaft",
            "Hilfsfonds",
            "Amtssitz",
            "Bundesstraße",
            "vielleicht",
            "Mannschaft",
            "Aufnahme",
            "Kenntnis",
            "Standard",
            "Rückgrat",
        ] {
            assert_eq!(correction(word), None, "{word} should be left alone");
        }
    }
}
