//! The German linters, collected into one `LintGroup`.

use std::sync::Arc;

use harper_core::linting::LintGroup;
use harper_core::spell::Dictionary;

pub fn rust_lint_group(dictionary: Arc<impl Dictionary + 'static>) -> LintGroup {
    use crate::linting::{
        german_absolute_superlative::GermanAbsoluteSuperlative,
        german_adjective_form::GermanAdjectiveForm,
        german_common_typos::GermanCommonTypos,
        german_compound_adjective_capitalization::GermanCompoundAdjectiveCapitalization,
        german_coordinating_comma::GermanCoordinatingComma,
        german_country_article::GermanCountryArticle,
        german_dative_plural::GermanDativePlural,
        german_determiner_gender::GermanDeterminerGender,
        german_filler_words::GermanFillerWords,
        german_fixed_nominalization::GermanFixedNominalization,
        german_genitive_after_nominative_article::GermanGenitiveAfterNominativeArticle,
        german_modal_zu_infinitive::GermanModalZuInfinitive,
        german_nominalized_adjective::GermanNominalizedAdjective,
        german_nominalized_infinitive::GermanNominalizedInfinitive,
        german_noun_capitalization::GermanNounCapitalization,
        german_perfect_auxiliary::GermanPerfectAuxiliary,
        german_preposition_case::GermanPrepositionCase,
        german_question_mark::GermanQuestionMark,
        german_recommended_fusion::GermanRecommendedFusion,
        german_relative_clause_comma::GermanRelativeClauseComma,
        german_repeated_words::GermanRepeatedWords,
        german_sentence_capitalization::GermanSentenceCapitalization,
        german_spell_check::GermanSpellCheck,
        german_split_particle::GermanSplitParticle,
        german_strong_imperative::GermanStrongImperative,
        german_subject_verb_agreement::GermanSubjectVerbAgreement,
        german_subordinate_comma::GermanSubordinateComma,
        german_subordinate_word_order::GermanSubordinateWordOrder,
        german_suspended_hyphen::GermanSuspendedHyphen,
        german_typography::{GermanNumberFormat, GermanQuotationMarks},
        german_verb_cluster::GermanVerbCluster,
        german_wider_wieder::GermanWiderWieder,
        german_wording::{GermanDoubleComparison, GermanReasonIsBecause},
        german_year_preposition::GermanYearPreposition,
    };

    let mut group = LintGroup::empty();

    // Typography, borrowed whole from the shared core. Spacing around a
    // comma and doubled spaces are the same mistake in every language that
    // uses the Latin script, so German gets the two rules as they stand
    // rather than a copy of them. Their messages are English and stay
    // English: the suggestion is what the user acts on, and it is a
    // character, not a sentence. See `language/AGENTS.md` on reuse.
    group.add("Spaces", harper_core::linting::spaces::Spaces);
    group.add("CommaFixes", harper_core::linting::comma_fixes::CommaFixes);

    group.add(
        "GermanSpellCheck",
        GermanSpellCheck::new(dictionary.clone()),
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
    group.add("GermanSubordinateComma", GermanSubordinateComma);
    group.add("GermanCoordinatingComma", GermanCoordinatingComma);
    group.add("GermanNumberFormat", GermanNumberFormat);
    group.add("GermanQuotationMarks", GermanQuotationMarks);
    group.add("GermanReasonIsBecause", GermanReasonIsBecause);
    group.add("GermanDoubleComparison", GermanDoubleComparison);
    group.add("GermanSubordinateWordOrder", GermanSubordinateWordOrder);
    group.add("GermanVerbCluster", GermanVerbCluster::new());
    group.add("GermanSuspendedHyphen", GermanSuspendedHyphen);
    group.add(
        "GermanRelativeClauseComma",
        GermanRelativeClauseComma::new(),
    );
    group.add("GermanYearPreposition", GermanYearPreposition);
    group.add("GermanPerfectAuxiliary", GermanPerfectAuxiliary);
    group.add("GermanModalZuInfinitive", GermanModalZuInfinitive);
    group.add("GermanStrongImperative", GermanStrongImperative);
    group.add("GermanQuestionMark", GermanQuestionMark);
    group.add("GermanCountryArticle", GermanCountryArticle);
    group.add("GermanAdjectiveForm", GermanAdjectiveForm::new());
    group.add("GermanPrepositionCase", GermanPrepositionCase::new());
    group.add("GermanDativePlural", GermanDativePlural::new());
    group.add("GermanDeterminerGender", GermanDeterminerGender::new());
    group.add(
        "GermanGenitiveAfterNominativeArticle",
        GermanGenitiveAfterNominativeArticle::new(),
    );
    group
}
