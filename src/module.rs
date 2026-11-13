//! The German linters, collected into one `LintGroup`.

use std::sync::Arc;

use harper_core::linting::LintGroup;
use harper_core::spell::Dictionary;

use crate::dialects::GermanDialect;

pub fn rust_lint_group(
    dictionary: Arc<impl Dictionary + 'static>,
    dialect: GermanDialect,
) -> LintGroup {
    use crate::linting::{
        german_absolute_superlative::GermanAbsoluteSuperlative,
        german_adjective_form::GermanAdjectiveForm,
        german_bare_stem::GermanBareStem,
        german_colloquial::GermanColloquial,
        german_common_typos::GermanCommonTypos,
        german_comparative_clause_comma::GermanComparativeClauseComma,
        german_compound_adjective_capitalization::GermanCompoundAdjectiveCapitalization,
        german_coordinated_subject::GermanCoordinatedSubject,
        german_coordinating_comma::GermanCoordinatingComma,
        german_country_article::GermanCountryArticle,
        german_dass_article::GermanDassArticle,
        german_dative_plural::GermanDativePlural,
        german_determiner_gender::GermanDeterminerGender,
        german_double_negation::GermanDoubleNegation,
        german_doubled_verb::GermanDoubledVerb,
        german_filler_words::GermanFillerWords,
        german_fixed_nominalization::GermanFixedNominalization,
        german_genitive_after_nominative_article::GermanGenitiveAfterNominativeArticle,
        german_genitive_s::GermanGenitiveS,
        german_masculine_object::GermanMasculineObject,
        german_mid_sentence_capital::GermanMidSentenceCapital,
        german_missing_article::GermanMissingArticle,
        german_missing_zu::GermanMissingZu,
        german_modal_finite_sein::GermanModalFiniteSein,
        german_modal_zu_infinitive::GermanModalZuInfinitive,
        german_nominalized_adjective::GermanNominalizedAdjective,
        german_nominalized_infinitive::GermanNominalizedInfinitive,
        german_noun_capitalization::GermanNounCapitalization,
        german_numeral_plural::GermanNumeralPlural,
        german_partitive_subject::GermanPartitiveSubject,
        german_perfect_auxiliary::GermanPerfectAuxiliary,
        german_polite_sie::GermanPoliteSie,
        german_preposition_case::GermanPrepositionCase,
        german_preposition_pronoun::GermanPrepositionPronoun,
        german_question_mark::GermanQuestionMark,
        german_question_word_order::GermanQuestionWordOrder,
        german_recommended_fusion::GermanRecommendedFusion,
        german_relative_clause_comma::GermanRelativeClauseComma,
        german_relative_pronoun::GermanRelativePronoun,
        german_repeated_words::GermanRepeatedWords,
        german_sentence_capitalization::GermanSentenceCapitalization,
        german_sonst_comma::GermanSonstComma,
        german_spacing::{GermanCommaFixes, GermanDoublePeriod, GermanSpaces},
        german_spell_check::GermanSpellCheck,
        german_split_compound::GermanSplitCompound,
        german_split_particle::GermanSplitParticle,
        german_strong_imperative::GermanStrongImperative,
        german_subject_verb_agreement::GermanSubjectVerbAgreement,
        german_subordinate_comma::GermanSubordinateComma,
        german_subordinate_word_order::GermanSubordinateWordOrder,
        german_suspended_hyphen::GermanSuspendedHyphen,
        german_typography::{
            GermanApostrophe, GermanCoupling, GermanNumberFormat, GermanQuotationMarks,
        },
        german_verb_case::GermanVerbCase,
        german_verb_cluster::GermanVerbCluster,
        german_verb_preposition::GermanVerbPreposition,
        german_verb_second::GermanVerbSecond,
        german_viel_plural::GermanVielPlural,
        german_vorfeld_comma::GermanVorfeldComma,
        german_weak_adjective::GermanWeakAdjective,
        german_weak_noun::GermanWeakNoun,
        german_wider_wieder::GermanWiderWieder,
        german_wording::{
            GermanDoubleComparison, GermanDoublePassive, GermanPassivePerfect,
            GermanReasonIsBecause,
        },
        german_written_together::GermanWrittenTogether,
        german_year_preposition::GermanYearPreposition,
    };

    let mut group = LintGroup::empty();

    // Typography from the shared core: spacing around a comma and doubled
    // spaces are the same mistake in every language that uses the Latin
    // script. The wrappers translate the core's messages and add the gap
    // between two sentences, which German typography fixes at one space.
    // The rule names stay the core's, so existing configurations still apply.
    group.add("Spaces", GermanSpaces::default());
    group.add("CommaFixes", GermanCommaFixes::default());
    group.add("GermanDoublePeriod", GermanDoublePeriod);
    group.add("GermanDassArticle", GermanDassArticle::new());

    group.add(
        "GermanSpellCheck",
        GermanSpellCheck::for_dialect(dictionary.clone(), dialect),
    );
    group.add(
        "GermanNounCapitalization",
        GermanNounCapitalization::new(dictionary.clone()),
    );
    group.add(
        "GermanSentenceCapitalization",
        GermanSentenceCapitalization::new(dictionary.clone()),
    );
    group.add(
        "GermanSubjectVerbAgreement",
        GermanSubjectVerbAgreement::new(dictionary.clone()),
    );
    group.add("GermanFillerWords", GermanFillerWords::default());
    group.add("GermanRecommendedFusion", GermanRecommendedFusion);
    group.add("GermanWiderWieder", GermanWiderWieder);
    group.add("GermanWrittenTogether", GermanWrittenTogether);
    group.add("GermanRepeatedWords", GermanRepeatedWords);
    group.add("GermanSplitParticle", GermanSplitParticle);
    group.add("GermanCommonTypos", GermanCommonTypos);
    group.add(
        "GermanCompoundAdjectiveCapitalization",
        GermanCompoundAdjectiveCapitalization::new(dictionary.clone()),
    );
    group.add("GermanAbsoluteSuperlative", GermanAbsoluteSuperlative);
    group.add("GermanFixedNominalization", GermanFixedNominalization);
    group.add("GermanNominalizedInfinitive", GermanNominalizedInfinitive);
    group.add("GermanNominalizedAdjective", GermanNominalizedAdjective);
    group.add("GermanVerbCase", GermanVerbCase::default());
    group.add(
        "GermanMidSentenceCapital",
        GermanMidSentenceCapital::default(),
    );
    group.add("GermanSubordinateComma", GermanSubordinateComma);
    group.add("GermanCoordinatingComma", GermanCoordinatingComma);
    group.add("GermanVorfeldComma", GermanVorfeldComma);
    group.add("GermanWeakNoun", GermanWeakNoun);
    group.add("GermanGenitiveS", GermanGenitiveS::new());
    group.add("GermanDoubleNegation", GermanDoubleNegation);
    group.add("GermanComparativeClauseComma", GermanComparativeClauseComma);
    group.add("GermanNumberFormat", GermanNumberFormat);
    group.add("GermanCoupling", GermanCoupling);
    group.add("GermanApostrophe", GermanApostrophe);
    group.add("GermanColloquial", GermanColloquial);
    group.add("GermanQuotationMarks", GermanQuotationMarks);
    group.add("GermanReasonIsBecause", GermanReasonIsBecause);
    group.add(
        "GermanVerbSecond",
        GermanVerbSecond::new(dictionary.clone()),
    );
    group.add("GermanVerbPreposition", GermanVerbPreposition::new());
    group.add(
        "GermanSplitCompound",
        GermanSplitCompound::new(crate::spell::base_german_dictionary()),
    );
    group.add("GermanDoubleComparison", GermanDoubleComparison);
    group.add("GermanDoublePassive", GermanDoublePassive);
    group.add("GermanPassivePerfect", GermanPassivePerfect);
    group.add("GermanSubordinateWordOrder", GermanSubordinateWordOrder);
    group.add("GermanVerbCluster", GermanVerbCluster::new());
    group.add("GermanSuspendedHyphen", GermanSuspendedHyphen);
    group.add(
        "GermanRelativeClauseComma",
        GermanRelativeClauseComma::new(),
    );
    group.add("GermanRelativePronoun", GermanRelativePronoun::default());
    group.add("GermanPrepositionPronoun", GermanPrepositionPronoun);
    group.add("GermanYearPreposition", GermanYearPreposition);
    group.add("GermanPerfectAuxiliary", GermanPerfectAuxiliary);
    group.add("GermanModalZuInfinitive", GermanModalZuInfinitive);
    group.add("GermanModalFiniteSein", GermanModalFiniteSein);
    group.add("GermanStrongImperative", GermanStrongImperative);
    group.add("GermanQuestionMark", GermanQuestionMark);
    group.add("GermanCountryArticle", GermanCountryArticle);
    group.add("GermanAdjectiveForm", GermanAdjectiveForm::new());
    group.add("GermanPrepositionCase", GermanPrepositionCase::new());
    group.add("GermanDativePlural", GermanDativePlural::new());
    group.add("GermanNumeralPlural", GermanNumeralPlural::new());
    group.add("GermanBareStem", GermanBareStem::default());
    group.add("GermanMissingZu", GermanMissingZu);
    group.add("GermanQuestionWordOrder", GermanQuestionWordOrder);
    group.add(
        "GermanCoordinatedSubject",
        GermanCoordinatedSubject::default(),
    );
    group.add("GermanPoliteSie", GermanPoliteSie);
    group.add("GermanMissingArticle", GermanMissingArticle::default());
    group.add("GermanWeakAdjective", GermanWeakAdjective::default());
    group.add("GermanDoubledVerb", GermanDoubledVerb);
    group.add("GermanSonstComma", GermanSonstComma);
    group.add("GermanVielPlural", GermanVielPlural::default());
    group.add("GermanMasculineObject", GermanMasculineObject::default());
    group.add("GermanPartitiveSubject", GermanPartitiveSubject::default());
    group.add("GermanDeterminerGender", GermanDeterminerGender::new());
    group.add(
        "GermanGenitiveAfterNominativeArticle",
        GermanGenitiveAfterNominativeArticle::new(),
    );
    group
}
