//! German linting rules and checkers.

pub mod german_absolute_superlative;
pub mod german_adjective_form;
pub mod german_common_typos;
pub mod german_compound_adjective_capitalization;
pub mod german_coordinating_comma;
pub mod german_country_article;
pub mod german_dative_plural;
pub mod german_determiner_gender;
pub mod german_filler_words;
pub mod german_fixed_nominalization;
pub mod german_foreign_stretch;
pub mod german_genitive_after_nominative_article;
pub mod german_modal_zu_infinitive;
pub mod german_nominalized_adjective;
pub mod german_nominalized_infinitive;
pub mod german_noun_capitalization;
pub mod german_perfect_auxiliary;
pub mod german_preposition_case;
pub mod german_question_mark;
pub mod german_recommended_fusion;
pub mod german_relative_clause_comma;
pub mod german_repeated_words;
pub mod german_sentence_capitalization;
pub mod german_spell_check;
pub mod german_split_particle;
pub mod german_strong_imperative;
pub mod german_subject_verb_agreement;
pub mod german_subordinate_comma;
pub mod german_subordinate_word_order;
pub mod german_suspended_hyphen;
pub mod german_typography;
pub mod german_verb_cluster;
pub mod german_wider_wieder;
pub mod german_wording;
pub mod german_year_preposition;
pub mod weir_rules;

use harper_core::linting::LintGroup;
use harper_core::spell::Dictionary;
use std::sync::Arc;

use crate::dialects::GermanDialect;
use crate::module::rust_lint_group;

/// The German rules, all enabled: the Weir rules and the Rust linters.
///
/// The dialect does not change which rules run; it is kept so that callers can say which one
/// they mean.
pub fn new_curated_german(
    _dialect: GermanDialect,
    dictionary: Arc<impl Dictionary + 'static>,
) -> LintGroup {
    let mut group = LintGroup::empty();
    group.merge_from(weir_rules::lint_group());
    group.merge_from(rust_lint_group(dictionary));
    group.set_all_rules_to(Some(true));

    group
}
