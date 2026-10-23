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
const HARPER_CORE_VERSION: &str = "2.13.0";

pub struct German {
    base: Arc<dyn Dictionary>,
    /// The dictionary the linters were built with: `base`, plus the user's words.
    dictionary: Arc<MergedDictionary>,
    /// The user words `dictionary` and `group` were built with.
    words: Vec<String>,
    /// The dialect `group` was built for.
    dialect: GermanDialect,
    group: LintGroup,
}

impl German {
    fn build_group(dictionary: &Arc<MergedDictionary>, dialect: GermanDialect) -> LintGroup {
        new_curated_german(dialect, dictionary.clone())
    }

    /// Lint as `dialect`, an id, name or alias of [`PluginInfo::dialects`]; as the standard
    /// German of Germany when it is none of them, or absent.
    fn use_dialect(&mut self, dialect: Option<&str>) {
        let dialect = dialect
            .and_then(|name| Self::info().dialect_named(name).cloned())
            .and_then(|info| info.id.parse::<GermanDialect>().ok())
            .unwrap_or_default();
        if dialect != self.dialect {
            let config: FlatConfig = self.group.config.clone();
            self.group = Self::build_group(&self.dictionary, dialect).with_lint_config(config);
            self.dialect = dialect;
        }
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

    /// Make `words` the user words, rebuilding the linters only if they differ from the ones in use.
    fn use_words(&mut self, words: &[String]) {
        if self.words == words {
            return;
        }
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
        self.group = Self::build_group(&merged, self.dialect).with_lint_config(config);
        self.dictionary = merged;
        self.words = words.to_vec();
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

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| n.to_string()).collect()
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
                    aliases: names(&["Germany", "de-DE", "de_DE"]),
                },
                DialectInfo {
                    id: "Austrian".into(),
                    name: "Österreich".into(),
                    aliases: names(&["Austria", "AT", "de-AT", "de_AT"]),
                },
                DialectInfo {
                    id: "Swiss".into(),
                    name: "Schweiz".into(),
                    aliases: names(&["Switzerland", "CH", "de-CH", "de_CH"]),
                },
            ],
            default_dialect: Some("Standard".into()),
            // What the multilingual build of Harper accepted for German, so its settings keep
            // working.
            aliases: names(&["German", "Deutsch"]),
            // The API crate adds what this build can do (`request_words`).
            features: Vec::new(),
        }
    }

    fn new() -> Self {
        let base: Arc<dyn Dictionary> = compound_aware_german_dictionary();
        let mut merged = MergedDictionary::new();
        merged.add_dictionary(base.clone());
        let dictionary = Arc::new(merged);
        Self {
            group: Self::build_group(&dictionary, GermanDialect::Standard),
            base,
            dictionary,
            words: Vec::new(),
            dialect: GermanDialect::Standard,
        }
    }

    fn lint(&mut self, req: LintRequest) -> Vec<PluginLint> {
        // The words belong to the request, so a host may lint several files in any order.
        if let Some(words) = &req.user_words {
            self.use_words(words);
        }
        // Like the words, the dialect belongs to the request.
        self.use_dialect(req.dialect.as_deref());
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

    fn detect_with(text: &str, english_ratio: Option<f32>) -> Option<f32> {
        match english_ratio {
            Some(ratio) => crate::language_detection::german_confidence_given_english(text, ratio),
            None => Self::detect(text),
        }
    }

    fn set_user_words(&mut self, words: Vec<String>) {
        self.use_words(&words);
    }
}

export_plugin!(German);

#[cfg(test)]
mod tests {
    use harper_plugin_api::{LanguagePlugin, LintRequest};

    use super::German;

    /// Hosts find the plugin by these names, e.g. a harper-ls setting `"dialect": "German"`.
    #[test]
    fn answers_to_its_names_and_dialects() {
        let info = German::info();
        for name in [
            "de", "Deutsch", "german", "Swiss", "Schweiz", "de-AT", "Germany",
        ] {
            assert!(info.answers_to(name), "{name}");
        }
        assert!(!info.answers_to("English"));
        assert_eq!(info.dialect_named("ch").unwrap().id, "Swiss");
    }

    #[test]
    fn lints_in_the_dialect_of_the_request() {
        let mut german = German::new();
        let text = "Es heisst, die Strasse sei gesperrt.";
        let spelling = |german: &mut German, dialect: Option<&str>| {
            let mut req = LintRequest::plain(text);
            req.dialect = dialect.map(str::to_owned);
            german
                .lint(req)
                .into_iter()
                .filter(|lint| lint.rule == "GermanSpellCheck")
                .count()
        };
        assert_eq!(spelling(&mut german, None), 2);
        assert_eq!(spelling(&mut german, Some("de-CH")), 0);
        assert_eq!(spelling(&mut german, None), 2);
        assert_eq!(spelling(&mut german, Some("Schweiz")), 0);
        assert_eq!(spelling(&mut german, Some("Klingonisch")), 2);
    }
}
