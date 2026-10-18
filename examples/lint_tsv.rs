//! Lints many sentences in one process, for measuring a change to a rule: reads `id<TAB>text`
//! lines from the file in the first argument and prints one JSON object per lint.
//!
//!     CARGO_PROFILE_RELEASE_LTO=false cargo run --release --example lint_tsv -- sentences.tsv
//!
//! Compare two runs by `id`, rule and span, not by message: the order of spelling suggestions is
//! not reproducible.

use harper_german::plugin::German;
use harper_plugin_api::{LanguagePlugin, LintRequest};

fn main() {
    let path = std::env::args().nth(1).expect("usage: lint_tsv FILE");
    let input = std::fs::read_to_string(path).expect("cannot read the file");
    let mut german = German::new();
    for line in input.lines() {
        let Some((id, text)) = line.split_once('\t') else {
            continue;
        };
        let chars: Vec<char> = text.chars().collect();
        for lint in german.lint(LintRequest::plain(text)) {
            let problem: String = chars[lint.span.0..lint.span.1].iter().collect();
            println!(
                "{}",
                serde_json::json!({
                    "id": id,
                    "rule": lint.rule,
                    "start": lint.span.0,
                    "end": lint.span.1,
                    "problem": problem,
                    "suggestions": lint.suggestions.len(),
                })
            );
        }
    }
}
