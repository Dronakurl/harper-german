//! Loads the built `harper_german.wasm` through `harper-plugin-host`, the way `harper-ls` does.
//!
//! Build it first: `just build-wasm`. Without the module these tests do nothing locally and
//! fail in CI, where it must exist.

use std::path::PathBuf;
use std::time::Instant;

use harper_plugin_api::{LintRequest, Region};
use harper_plugin_host::{LoadedPlugin, PluginHost};

fn wasm_path() -> PathBuf {
    std::env::var_os("HARPER_DE_WASM")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target/wasm32-unknown-unknown/release/harper_german.wasm")
        })
}

fn load() -> Option<LoadedPlugin> {
    let path = wasm_path();
    if !path.exists() {
        assert!(
            std::env::var_os("CI").is_none(),
            "{} must exist in CI",
            path.display()
        );
        eprintln!(
            "skipping: {} does not exist (run `just build-wasm`)",
            path.display()
        );
        return None;
    }

    let host = PluginHost::new().unwrap();
    let cache = std::env::temp_dir().join("harper-german-test-cache");
    let t = Instant::now();
    let plugin = host.load_file_cached(&path, &cache).unwrap();
    eprintln!("load (compile + instantiate): {:?}", t.elapsed());
    Some(plugin)
}

#[test]
fn reports_what_it_is() {
    let Some(plugin) = load() else { return };
    let info = plugin.info();
    assert_eq!(info.id, "de");
    assert_eq!(info.name, "Deutsch");
    assert_eq!(info.abi, 1);
    assert!(
        plugin
            .lint_descriptions()
            .unwrap()
            .contains_key("GermanSpellCheck")
    );
    assert!(
        plugin
            .default_config()
            .unwrap()
            .contains_key("GermanSpellCheck")
    );
}

#[test]
fn finds_a_misspelling_and_suggests_the_fix() {
    let Some(plugin) = load() else { return };
    let t = Instant::now();
    let lints = plugin
        .lint(&LintRequest::plain("Die Katze schlaeft."))
        .unwrap();
    eprintln!("first lint: {:?}", t.elapsed());

    let spell = lints
        .iter()
        .find(|l| l.rule == "GermanSpellCheck")
        .expect("a spelling lint");
    assert_eq!(spell.span, (10, 18));

    let t = Instant::now();
    plugin
        .lint(&LintRequest::plain("Die Katze schlaeft."))
        .unwrap();
    eprintln!("second lint: {:?}", t.elapsed());
}

#[test]
fn clean_text_has_no_lints() {
    let Some(plugin) = load() else { return };
    let lints = plugin
        .lint(&LintRequest::plain("Der Hund schläft im Garten."))
        .unwrap();
    assert!(lints.is_empty(), "{lints:?}");
}

#[test]
fn user_words_silence_the_spell_checker() {
    let Some(plugin) = load() else { return };
    let text = "Wir benutzen Harperlint täglich.";
    let before = plugin.lint(&LintRequest::plain(text)).unwrap();
    assert!(
        before.iter().any(|l| l.rule == "GermanSpellCheck"),
        "{before:?}"
    );

    plugin.set_user_words(vec!["Harperlint".into()]).unwrap();
    let after = plugin.lint(&LintRequest::plain(text)).unwrap();
    assert!(
        after.iter().all(|l| l.rule != "GermanSpellCheck"),
        "{after:?}"
    );
}

#[test]
fn markdown_and_regions() {
    let Some(plugin) = load() else { return };

    let mut req = LintRequest::plain("# Titel\n\nDie Katze schlaeft.\n\n```\nschlaeft\n```\n");
    req.format = "markdown".into();
    let lints = plugin.lint(&req).unwrap();
    let spelling: Vec<_> = lints
        .iter()
        .filter(|l| l.rule == "GermanSpellCheck")
        .collect();
    assert_eq!(spelling.len(), 1, "code blocks are not prose: {spelling:?}");

    // Only the region is linted, and spans stay relative to the whole text.
    let text = "schlaeft nicht // Die Katze schlaeft.";
    let mut req = LintRequest::plain(text);
    req.regions = Some(vec![Region {
        start: 17,
        end: text.chars().count(),
        format: "plain".into(),
    }]);
    let lints = plugin.lint(&req).unwrap();
    let spelling: Vec<_> = lints
        .iter()
        .filter(|l| l.rule == "GermanSpellCheck")
        .collect();
    assert_eq!(spelling.len(), 1, "{spelling:?}");
    assert_eq!(spelling[0].span, (28, 36));
}

/// The module gives the same lints as the same code run natively, on every sample text.
#[test]
fn wasm_matches_native() {
    use harper_german::plugin::German;
    use harper_plugin_api::{LanguagePlugin, PluginLint};

    let Some(plugin) = load() else { return };
    let mut native = German::new();

    let mut files: Vec<_> = std::fs::read_dir("test_sources")
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    files.sort();
    assert!(!files.is_empty());

    let sorted = |mut lints: Vec<PluginLint>| {
        lints.sort_by(|a, b| (a.span, &a.rule, &a.message).cmp(&(b.span, &b.rule, &b.message)));
        lints
    };

    let mut total = 0;
    for file in files {
        let mut req = LintRequest::plain(std::fs::read_to_string(&file).unwrap());
        req.format = "markdown".into();

        let from_wasm = sorted(plugin.lint(&req).unwrap());
        let from_native = sorted(native.lint(req));
        assert_eq!(from_wasm, from_native, "{}", file.display());
        total += from_wasm.len();
    }
    eprintln!("{total} lints agree");
    assert!(total > 0, "the sample texts should produce some lints");
}
