//! German as a Harper language plugin: the [`LanguagePlugin`] implementation and its WebAssembly
//! exports.

use std::collections::BTreeMap;
use std::sync::Arc;

use harper_core::linting::{FlatConfig, Lint, LintGroup};
use harper_core::parsers::{Markdown, MarkdownOptions, OrgMode, Parser};
use harper_core::spell::{Dictionary, MergedDictionary, MutableDictionary};
use harper_core::{DictWordMetadata, Document, remove_overlaps_map};
use harper_plugin_api::{
    ABI_VERSION, DialectInfo, LanguagePlugin, LintRequest, PluginConfig, PluginInfo, PluginLint,
    Region, export_plugin, lint_to_plugin,
};

use crate::dialects::GermanDialect;
use crate::linting::new_curated_german;
use crate::parsers::PlainGerman;
use crate::spell::compound_aware_german_dictionary;

/// The `harper-core` this plugin is built against; kept in step with `Cargo.toml`.
const HARPER_CORE_VERSION: &str = "2.12.0";

pub struct German {
    base: Arc<dyn Dictionary>,
    /// The dictionary the linters were built with: `base`, plus the user's words.
    dictionary: Arc<MergedDictionary>,
    group: LintGroup,
}

impl German {
    fn build_group(dictionary: &Arc<MergedDictionary>) -> LintGroup {
        new_curated_german(GermanDialect::Standard, dictionary.clone())
    }

    fn parser(format: &str) -> Box<dyn Parser> {
        match format {
            "markdown" => Box::new(
                Markdown::with_inline_parser(MarkdownOptions::default(), |source| {
                    PlainGerman.parse(source)
                })
                .non_english(),
            ),
            "org" => Box::new(
                OrgMode::with_inline_parser(|source| PlainGerman.parse(source)).non_english(),
            ),
            _ => Box::new(PlainGerman),
        }
    }

    /// Lint `chars` as `format` and report spans relative to `offset`.
    fn lint_chars(&mut self, chars: &[char], format: &str, offset: usize) -> Vec<PluginLint> {
        let parser = Self::parser(format);
        let document = Document::new_from_chars(chars.to_vec().into(), &parser, &*self.dictionary);

        let mut lints: BTreeMap<String, Vec<Lint>> = self.group.organized_lints(&document);
        remove_overlaps_map(&mut lints);

        lints
            .iter()
            .flat_map(|(rule, lints)| {
                lints.iter().map(move |lint| {
                    let mut lint = lint_to_plugin(lint, rule);
                    lint.span = (lint.span.0 + offset, lint.span.1 + offset);
                    lint
                })
            })
            .collect()
    }

    fn apply_config(&mut self, config: &PluginConfig) {
        for (rule, enabled) in config {
            if let Some(enabled) = enabled
                && self.group.contains_key(rule)
            {
                self.group.config.set_rule_enabled(rule, *enabled);
            }
        }
    }
}

impl LanguagePlugin for German {
    fn info() -> PluginInfo {
        PluginInfo {
            id: "de".into(),
            name: "Deutsch".into(),
            plugin_version: env!("CARGO_PKG_VERSION").into(),
            abi: ABI_VERSION,
            harper_core_version: HARPER_CORE_VERSION.into(),
            dialects: vec![
                DialectInfo {
                    id: "Standard".into(),
                    name: "Deutschland".into(),
                },
                DialectInfo {
                    id: "Austrian".into(),
                    name: "Österreich".into(),
                },
                DialectInfo {
                    id: "Swiss".into(),
                    name: "Schweiz".into(),
                },
            ],
            default_dialect: Some("Standard".into()),
        }
    }

    fn new() -> Self {
        let base: Arc<dyn Dictionary> = compound_aware_german_dictionary();
        let mut merged = MergedDictionary::new();
        merged.add_dictionary(base.clone());
        let dictionary = Arc::new(merged);
        Self {
            group: Self::build_group(&dictionary),
            base,
            dictionary,
        }
    }

    fn lint(&mut self, req: LintRequest) -> Vec<PluginLint> {
        if let Some(config) = &req.config {
            self.apply_config(config);
        }

        let chars: Vec<char> = req.text.chars().collect();

        match &req.regions {
            None => self.lint_chars(&chars, &req.format, 0),
            Some(regions) => regions
                .iter()
                .filter(|Region { start, end, .. }| start < end && *end <= chars.len())
                .flat_map(|region| {
                    self.lint_chars(
                        &chars[region.start..region.end],
                        &region.format,
                        region.start,
                    )
                })
                .collect(),
        }
    }

    fn lint_descriptions(&self) -> BTreeMap<String, String> {
        self.group
            .all_descriptions()
            .into_iter()
            .map(|(rule, description)| (rule.to_owned(), description.to_owned()))
            .collect()
    }

    fn default_config(&self) -> PluginConfig {
        self.group
            .iter_keys()
            .map(|rule| (rule.to_owned(), Some(true)))
            .collect()
    }

    fn detect(text: &str) -> Option<f32> {
        crate::language_detection::german_confidence(text)
    }

    fn set_user_words(&mut self, words: Vec<String>) {
        let mut user = MutableDictionary::new();
        user.extend_words(words.iter().map(|w| {
            (
                w.chars().collect::<Vec<char>>(),
                DictWordMetadata::default(),
            )
        }));

        let mut merged = MergedDictionary::new();
        merged.add_dictionary(self.base.clone());
        merged.add_dictionary(Arc::new(user));
        let merged = Arc::new(merged);

        // The linters hold on to the dictionary, so they are rebuilt with the new one. Keep the
        // rules the user switched off.
        let config: FlatConfig = self.group.config.clone();
        self.group = Self::build_group(&merged).with_lint_config(config);
        self.dictionary = merged;
    }
}

export_plugin!(German);
