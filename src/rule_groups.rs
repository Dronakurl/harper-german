//! The themes the German rules are listed under in a host's settings, like the categories of the
//! English rules. A rule that is in no theme is listed on its own, so a new rule shows up without
//! being added here, but `every_rule_is_in_exactly_one_group` keeps this table complete.

use harper_plugin_api::RuleGroup;

struct Theme {
    label: &'static str,
    description: &'static str,
    rules: &'static [&'static str],
}

const THEMES: &[Theme] = &[
    Theme {
        label: "Rechtschreibung",
        description: "Rechtschreibfehler, Tippfehler und falsch geschriebene Wörter.",
        rules: &[
            "GermanSpellCheck",
            "GermanCommonTypos",
            "GermanBareStem",
            "LiessLies",
            "GermanSmallSlips",
            "Bisschen",
            "BenuetzenBenutzen",
            "VerlasenVerlassen",
            "WeisWeiss",
            "FliesendFliessend",
            "Zwingendermaassen",
            "VielLeicht",
            "SeiheSiehe",
            "Naemlich",
            "Nichtsdestotrotz",
            "Sowieso",
        ],
    },
    Theme {
        label: "Groß- und Kleinschreibung",
        description: "Großschreibung von Substantiven, Satzanfängen und Nominalisierungen.",
        rules: &[
            "GermanNounCapitalization",
            "GermanSentenceCapitalization",
            "GermanCompoundAdjectiveCapitalization",
            "GermanMidSentenceCapital",
            "GermanCapitalIhr",
            "GermanVerbCase",
            "GermanFixedNominalization",
            "GermanNominalizedInfinitive",
            "GermanNominalizedAdjective",
            "EMailKlein",
        ],
    },
    Theme {
        label: "Zeichensetzung und Leerzeichen",
        description: "Kommas, Anführungszeichen, Apostrophe, Bindestriche und Abstände.",
        rules: &[
            "Spaces",
            "CommaFixes",
            "GermanDoublePeriod",
            "GermanSubordinateComma",
            "GermanCoordinatingComma",
            "GermanVorfeldComma",
            "GermanRelativeClauseComma",
            "GermanComparativeClauseComma",
            "GermanSonstComma",
            "GermanApostrophe",
            "GermanQuotationMarks",
            "GermanQuestionMark",
            "GermanSuspendedHyphen",
            "EMailLeerzeichen",
        ],
    },
    Theme {
        label: "Zahlen und Zeiten",
        description: "Schreibweise von Zahlen, Jahreszahlen und Zahlwörtern.",
        rules: &[
            "GermanNumberFormat",
            "GermanYearPreposition",
            "GermanNumeralPlural",
            "Zweimal",
            "Dreimal",
            "Viermal",
            "Fuenfmal",
            "Zehnmal",
            "Hundertmal",
            "Tausendmal",
            "EineMillionen",
            "EineMilliarden",
        ],
    },
    Theme {
        label: "Zusammen- und Getrenntschreibung",
        description: "Wörter, die zusammen, getrennt oder mit Bindestrich geschrieben werden.",
        rules: &[
            "GermanSplitCompound",
            "GermanSplitParticle",
            "GermanWrittenTogether",
            "GermanRecommendedFusion",
            "GermanCoupling",
            "ZumAnbeissen",
            "WieViele",
        ],
    },
    Theme {
        label: "Pronomen und Anrede",
        description: "Pronomen, Relativsätze und die Anrede mit Du, Ihr und Sie.",
        rules: &[
            "GermanRelativePronoun",
            "GermanPrepositionPronoun",
            "GermanPoliteSie",
            "DativIhm",
            "IhrSeid",
            "SeidIhr",
            "ReflexivUns",
            "RelativDas",
            "VergleichNominativDu",
            "VergleichNominativIch",
            "DuWeisst",
            "ErLiest",
            "DeinDein",
            "MeinMein",
            "WirHaben",
            "AllesWas",
        ],
    },
    Theme {
        label: "Grammatik und Kongruenz",
        description: "Übereinstimmung von Subjekt und Verb, Artikel, Fall, Geschlecht und Plural.",
        rules: &[
            "GermanSubjectVerbAgreement",
            "GermanCoordinatedSubject",
            "GermanDeterminerGender",
            "GermanAdjectiveForm",
            "GermanWeakAdjective",
            "GermanWeakNoun",
            "GermanGenitiveS",
            "GermanGenitiveAfterNominativeArticle",
            "GermanDativePlural",
            "GermanMasculineObject",
            "GermanMissingArticle",
            "GermanCountryArticle",
            "GermanPrepositionCase",
            "GermanVerbPreposition",
            "GermanPartitiveSubject",
            "GermanVielPlural",
            "PluralFirmas",
            "PluralKaktusse",
            "PluralPraktikas",
            "PluralThemas",
            "PluralVisas",
            "ModalIst",
        ],
    },
    Theme {
        label: "Verben und Satzbau",
        description: "Verbstellung, Hilfsverben, Infinitive und die Reihenfolge der Satzglieder.",
        rules: &[
            "GermanVerbSecond",
            "GermanSubordinateWordOrder",
            "GermanQuestionWordOrder",
            "GermanVerbCluster",
            "GermanDoubledVerb",
            "GermanPerfectAuxiliary",
            "GermanSeinHaben",
            "GermanModalZuInfinitive",
            "GermanModalFiniteSein",
            "GermanMissingZu",
            "GermanStrongImperative",
        ],
    },
    Theme {
        label: "Verwechslungen",
        description: "Wörter, die verwechselt werden, etwa das und dass, seid und seit.",
        rules: &[
            "GermanDassArticle",
            "GermanWiderWieder",
            "Dass",
            "DasDass",
            "DassDas",
            "DassBitte",
            "EtwasDass",
            "SoDass",
            "BeimBei",
            "DennDen",
            "WiderAlsWieder",
            "SeidSeit",
            "SeitDem",
            "WahrenWaren",
            "WahrHaben",
            "RadRat",
            "LeerLehr",
            "StilStill",
            "WahlWal",
            "MieneMine",
            "SaiteSeite",
            "MahlenMalen",
            "GemahlenGemalt",
            "LidLied",
            "SonderSondern",
            "VielFiel",
            "GenantGenannt",
            "GilbtGilt",
            "GluckGlueck",
            "BissBis",
            "BerietenBereiten",
            "VerleihenVerleihen",
            "FuerFuer",
            "InDemIm",
            "ImAm",
            "ZuDemZum",
            "ZurDer",
            "ZurZum",
            "WennAls",
            "AlsWie",
            "VergleichAls",
            "KomparativWie",
            "Nachdem",
            "Wiederum",
            "GeschehenGeschehen",
            "SchuldSein",
            "BabyphoneBabyfon",
            "ChannelChannel",
        ],
    },
    Theme {
        label: "Stil und Redundanz",
        description: "Füllwörter, Wiederholungen, doppelte Verneinung und umständliche Formulierungen.",
        rules: &[
            "GermanFillerWords",
            "GermanRepeatedWords",
            "InIn",
            "GermanDoubleNegation",
            "GermanDoubleComparison",
            "GermanDoublePassive",
            "GermanPassivePerfect",
            "GermanReasonIsBecause",
            "GermanAbsoluteSuperlative",
            "GarKein",
            "GarKeine",
            "GarKeinen",
            "GarNicht",
            "GarNichts",
            "NieKein",
            "NiemandNichts",
            "GenauExakt",
            "GenausoWie",
            "Umso",
            "Irgendetwas",
            "Irgendjemand",
            "Irgendwann",
            "Irgendwo",
            "Ueberall",
            "Zuerst",
            "Zuletzt",
            "Vorher",
            "Nachher",
            "PaarEinige",
            "ZuLange",
            "ZuViel",
            "ZuWenig",
        ],
    },
    Theme {
        label: "Feste Wendungen",
        description: "Redewendungen und feste Verbindungen, die in einer bestimmten Form stehen.",
        rules: &[
            "AchtGeben",
            "AngstHaben",
            "AngstUndBange",
            "AufJedenFall",
            "BisAufWeiteres",
            "DesOefteren",
            "DesOefterenZusammen",
            "DesWeiteren",
            "ImAnbetracht",
            "ImVoraus",
            "LeidTun",
            "PleiteGegangen",
            "PleiteSein",
            "TotSein",
            "TotUmgefallen",
            "StolzAuf",
            "AngstVor",
            "ErachtensNach",
            "WartenAuf",
            "AbhaengenVon",
            "AbhaengenVom",
            "EssenGehen",
            "InteressierenFuer",
            "Inwiefern",
            "JeNachdem",
            "VorAllem",
            "EndeDiesesJahres",
            "LetztesEndes",
            "LetztenAdjektiv",
            "SinnErgeben",
            "SinnErgebenPlural",
            "SoAlsWie",
            "AlsErster",
            "AlsErste",
            "AlsLetzter",
            "AlsLetzte",
            "BereitsSchon",
        ],
    },
    Theme {
        label: "Briefe und Höflichkeitsformen",
        description: "Anreden, Grußformeln und Dankesformeln in Briefen und E-Mails.",
        rules: &[
            "AllesLiebe",
            "SehrGeehrteFrau",
            "SehrGeehrterHerr",
            "VielenDank",
            "HerzlichenDank",
            "HerzlichWillkommen",
            "DanksDanke",
            "Adresse",
            "EMail",
            "Diskussion",
            "Diskussionen",
            "Voraussetzung",
            "Voraussetzungen",
            "Standard",
            "IstWahr",
            "VereinigteStaaten",
        ],
    },
    Theme {
        label: "Umgangssprache",
        description: "Umgangssprachliche Wörter und Wendungen, die in geschriebenem Text auffallen.",
        rules: &["GermanColloquial", "OllesAlles"],
    },
];

/// The themes as the plugin API hands them to a host.
pub fn rule_groups() -> Vec<RuleGroup> {
    THEMES
        .iter()
        .map(|theme| RuleGroup {
            label: theme.label.to_owned(),
            description: theme.description.to_owned(),
            rules: theme.rules.iter().map(|rule| (*rule).to_owned()).collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use harper_plugin_api::LanguagePlugin;

    use crate::plugin::German;

    use super::THEMES;

    fn all_rules() -> BTreeSet<String> {
        German::new().default_config().into_keys().collect()
    }

    #[test]
    fn every_rule_is_in_exactly_one_group() {
        let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
        for theme in THEMES {
            for rule in theme.rules {
                if let Some(first) = seen.insert(rule, theme.label) {
                    panic!("{rule} is in \"{first}\" and in \"{}\"", theme.label);
                }
            }
        }

        let missing: Vec<_> = all_rules()
            .into_iter()
            .filter(|rule| !seen.contains_key(rule.as_str()))
            .collect();
        assert!(missing.is_empty(), "rules in no group: {missing:?}");
    }

    #[test]
    fn every_name_in_a_group_is_a_rule() {
        let rules = all_rules();
        let unknown: Vec<_> = THEMES
            .iter()
            .flat_map(|theme| theme.rules.iter())
            .filter(|rule| !rules.contains(**rule))
            .collect();
        assert!(unknown.is_empty(), "no such rule: {unknown:?}");
    }

    #[test]
    fn the_plugin_hands_the_groups_to_the_host() {
        let groups = German::rule_groups();
        assert_eq!(groups.len(), THEMES.len());
        assert!(
            groups
                .iter()
                .all(|g| !g.label.is_empty() && !g.rules.is_empty())
        );
    }
}
