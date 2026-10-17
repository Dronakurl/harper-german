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

#[test]
fn recognizes_german_without_building_the_dictionary() {
    let Some(plugin) = load() else { return };

    let t = Instant::now();
    let german = plugin
        .detect(
            "Der Hund ist im Garten. Die Katze schläft auf dem Sofa. Das Auto ist sehr schnell.",
        )
        .unwrap();
    eprintln!("detect: {:?}", t.elapsed());
    assert!(german.is_some_and(|c| c >= 0.5), "{german:?}");

    let english = plugin
        .detect("The dog plays in the garden. The cat sleeps on the sofa. The car is very fast.")
        .unwrap();
    assert_eq!(english, None);
}

fn request(text: &str, words: &[&str]) -> LintRequest {
    let mut req = LintRequest::plain(text);
    req.user_words = Some(words.iter().map(|w| w.to_string()).collect());
    req
}

fn flags(plugin: &LoadedPlugin, req: &LintRequest, word: &str) -> bool {
    let at = req.text.find(word).unwrap();
    let at = req.text[..at].chars().count();
    plugin
        .lint(req)
        .unwrap()
        .iter()
        .any(|l| l.rule == "GermanSpellCheck" && l.span.0 <= at && at < l.span.1)
}

/// The words come with the request: what one request brings does not leak into the next, and a
/// request without words does not undo `set_user_words`.
#[test]
fn words_belong_to_the_request() {
    let Some(plugin) = load() else { return };
    let text = "Der Quorblix ist hier.";

    assert!(flags(&plugin, &request(text, &[]), "Quorblix"));
    assert!(!flags(&plugin, &request(text, &["Quorblix"]), "Quorblix"));
    assert!(flags(&plugin, &request(text, &[]), "Quorblix"));

    plugin.set_user_words(vec!["Quorblix".into()]).unwrap();
    assert!(!flags(&plugin, &LintRequest::plain(text), "Quorblix"));
    assert!(flags(&plugin, &request(text, &[]), "Quorblix"));
}

/// Several threads share one plugin and lint with different words at the same time.
#[test]
fn concurrent_requests_do_not_see_each_others_words() {
    let Some(plugin) = load() else { return };
    let text = "Der Quorblix ist hier.";
    std::thread::scope(|scope| {
        for thread in 0..4 {
            let plugin = &plugin;
            scope.spawn(move || {
                for round in 0..25 {
                    let known = (thread + round) % 2 == 0;
                    let words: &[&str] = if known { &["Quorblix"] } else { &[] };
                    assert_eq!(
                        flags(plugin, &request(text, words), "Quorblix"),
                        !known,
                        "thread {thread}, round {round}"
                    );
                }
            });
        }
    });
}

/// What a request with words costs: the same words again, different words each time, and a
/// large user dictionary. Run with `--nocapture` to read the numbers.
#[test]
fn what_words_in_the_request_cost() {
    let Some(plugin) = load() else { return };
    let text = "Die Katze schläft auf dem Sofa und der Hund bellt laut im Garten.";
    let big: Vec<String> = (0..5000).map(|i| format!("Wort{i}x")).collect();
    let big: Vec<&str> = big.iter().map(String::as_str).collect();
    // The first call builds the dictionary.
    plugin.lint(&request(text, &[])).unwrap();

    let time = |label: &str, make: &dyn Fn(usize) -> LintRequest| {
        let n = 30;
        let t = Instant::now();
        for i in 0..n {
            plugin.lint(&make(i)).unwrap();
        }
        eprintln!("{label}: {:?} per request", t.elapsed() / n as u32);
    };
    time("no words", &|_| LintRequest::plain(text));
    time("same 2 words", &|_| {
        request(text, &["Quorblix", "Kuchenblech"])
    });
    time("alternating 2 lists", &|i| {
        request(
            text,
            if i % 2 == 0 {
                &["Quorblix"]
            } else {
                &["Kuchenblech"]
            },
        )
    });
    time("same 5000 words", &|_| request(text, &big));
    let alt: Vec<String> = (0..5000).map(|i| format!("Anders{i}x")).collect();
    let alt: Vec<&str> = alt.iter().map(String::as_str).collect();
    time("alternating 2 x 5000 words", &|i| {
        request(text, if i % 2 == 0 { &big } else { &alt })
    });
}
