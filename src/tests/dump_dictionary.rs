//! Dumps the expanded German dictionary with its metadata, for the scripts
//! that compare one build of `dictionary.dict` with another.
//!
//! ```bash
//! HARPER_DUMP_GERMAN_DICT=/tmp/de.tsv cargo test -p harper-core \
//!     --features multilingual --lib dump_german_dictionary -- --ignored
//! ```
//!
//! One line per word form: the spelling, a tab, the metadata as JSON.
//!
//! `probe_german_dictionary` asks the dictionary Harper actually spell-checks
//! with, compound acceptance included, about the words in a file:
//!
//! ```bash
//! HARPER_PROBE_IN=words.txt HARPER_PROBE_OUT=probe.tsv cargo test -p harper-core \
//!     --features multilingual --lib probe_german_dictionary -- --ignored
//! ```
//!
//! One line per input word: the word, a tab, and the metadata as JSON (empty
//! when the word is not accepted).

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::morphology::MorphologyExt;
    use crate::spell::curated_german_dictionary;
    use harper_core::DictWordMetadata;
    use harper_core::spell::Dictionary;

    /// The metadata as JSON, with the morphology readable rather than as
    /// `language_bits`, so the dumps keep the shape the scripts read.
    fn metadata_json(metadata: &DictWordMetadata) -> String {
        let mut value = serde_json::to_value(metadata).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("language_bits");
        if let Some(morphology) = metadata.morphology() {
            object.insert(
                "morphology".to_owned(),
                serde_json::to_value(morphology).unwrap(),
            );
        }
        value.to_string()
    }

    #[test]
    #[ignore = "writes a large file; run on demand"]
    fn dump_german_dictionary() {
        let Ok(path) = std::env::var("HARPER_DUMP_GERMAN_DICT") else {
            return;
        };
        let dict = curated_german_dictionary();
        let mut out = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
        for word in dict.words_iter() {
            let Some(metadata) = dict.get_word_metadata(word) else {
                continue;
            };
            let word: String = word.iter().collect();
            let json = metadata_json(&metadata);
            writeln!(out, "{word}\t{json}").unwrap();
        }
    }

    #[test]
    #[ignore = "reads and writes files named by the environment"]
    fn probe_german_dictionary() {
        let (Ok(input), Ok(output)) = (
            std::env::var("HARPER_PROBE_IN"),
            std::env::var("HARPER_PROBE_OUT"),
        ) else {
            return;
        };
        let dict = crate::spell::compound_aware_german_dictionary();
        let mut out = std::io::BufWriter::new(std::fs::File::create(output).unwrap());
        for word in std::fs::read_to_string(input).unwrap().lines() {
            let json = dict
                .get_word_metadata_str(word)
                .map(|metadata| metadata_json(&metadata))
                .unwrap_or_default();
            writeln!(out, "{word}\t{json}").unwrap();
        }
    }

    /// Prints what the base dictionary says about every word of `HARPER_DEBUG_TEXT`,
    /// the way the linters see it.
    #[test]
    #[ignore = "reads the text from the environment"]
    fn show_german_tokens() {
        use crate::parsers::PlainGerman;
        use harper_core::Document;

        let Ok(text) = std::env::var("HARPER_DEBUG_TEXT") else {
            return;
        };
        let dict = curated_german_dictionary();
        let document = Document::new(&text, &PlainGerman, &dict);
        if std::env::var("HARPER_DEBUG_LINT").is_ok() {
            use crate::dialects::GermanDialect;
            use crate::linting::new_curated_german;
            use harper_core::linting::Linter;

            let mut linter = new_curated_german(GermanDialect::Standard, dict.clone());
            for lint in linter.lint(&document) {
                println!(
                    "LINT {:?} {:?}: {}",
                    document.get_span_content_str(&lint.span),
                    lint.lint_kind,
                    lint.message
                );
            }
            return;
        }
        for token in document.tokens() {
            let word: String = document.get_span_content_str(&token.span);
            if let Some(metadata) = token.kind.as_word().and_then(|m| m.as_ref()) {
                let mut pos = Vec::new();
                if metadata.noun.is_some() {
                    pos.push("noun");
                }
                if metadata.verb.is_some() {
                    pos.push("verb");
                }
                if metadata.adjective.is_some() {
                    pos.push("adj");
                }
                if metadata.adverb.is_some() {
                    pos.push("adv");
                }
                if metadata.conjunction.is_some() {
                    pos.push("conj");
                }
                if metadata.determiner.is_some() {
                    pos.push("det");
                }
                if metadata.pronoun.is_some() {
                    pos.push("pron");
                }
                if metadata.preposition {
                    pos.push("prep");
                }
                let m = serde_json::to_string(&metadata.morphology()).unwrap();
                println!("{word}\t{} {m}", pos.join("+"));
            } else {
                println!("{word}\t-");
            }
        }
    }

    /// Lints the files named by `HARPER_LINT_FILE` (a file, or a directory whose
    /// files are all linted) with the curated German rules, and prints one
    /// `LINTJSON` line per lint: file, rule, char span, text and suggestions. It
    /// uses the plugin's dictionary and removes overlaps the way the plugin does,
    /// so it gives what `harper-cli --format json` gives for the built module,
    /// without a wasm build in between.
    #[test]
    #[ignore = "reads the files from the environment"]
    fn lint_file_as_json() {
        use crate::dialects::GermanDialect;
        use crate::linting::new_curated_german;
        use crate::parsers::PlainGerman;
        use harper_core::Document;

        let Ok(path) = std::env::var("HARPER_LINT_FILE") else {
            return;
        };
        let path = std::path::PathBuf::from(path);
        let mut files: Vec<_> = if path.is_dir() {
            std::fs::read_dir(&path)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect()
        } else {
            vec![path]
        };
        files.sort();

        let dict = crate::spell::compound_aware_german_dictionary();
        let mut linter = new_curated_german(GermanDialect::Standard, dict.clone());
        for file in files {
            let text = std::fs::read_to_string(&file).unwrap();
            let document = Document::new(&text, &PlainGerman, &*dict);
            let mut organized = linter.organized_lints(&document);
            harper_core::remove_overlaps_map(&mut organized);
            for (rule, lints) in organized {
                for lint in lints {
                    let suggestions: Vec<String> =
                        lint.suggestions.iter().map(|s| s.to_string()).collect();
                    println!(
                        "LINTJSON {}",
                        serde_json::json!({
                            "file": file.display().to_string(),
                            "rule": rule,
                            "span": {"char_start": lint.span.start, "char_end": lint.span.end},
                            "matched_text": document.get_span_content_str(&lint.span),
                            "suggestions": suggestions,
                        })
                    );
                }
            }
        }
    }

    /// Writes every spelling of the base dictionary, one per line, to the file
    /// named by `HARPER_DUMP_WORDS`.
    #[test]
    #[ignore = "writes a large file; run on demand"]
    fn dump_german_words() {
        let Ok(path) = std::env::var("HARPER_DUMP_WORDS") else {
            return;
        };
        let dict = curated_german_dictionary();
        let mut out = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
        for word in dict.words_iter() {
            let word: String = word.iter().collect();
            writeln!(out, "{word}").unwrap();
        }
    }

    /// For every word in the file `HARPER_COMPOUND_ONLY_IN`, prints it if the
    /// spell checker accepts it only as a compound, with the decomposition:
    /// `word<TAB>part|part|…`. Feeds the measurement of how a change to the
    /// compound rules would treat the words of a corpus.
    #[test]
    #[ignore = "reads the words from the environment"]
    fn list_compound_only_words() {
        let Ok(path) = std::env::var("HARPER_COMPOUND_ONLY_IN") else {
            return;
        };
        let dict = crate::spell::compound_aware_german_dictionary();
        let base = curated_german_dictionary();
        for word in std::fs::read_to_string(path).unwrap().lines() {
            let chars: Vec<char> = word.chars().collect();
            if chars.is_empty() || base.contains_word(&chars) || !dict.contains_word(&chars) {
                continue;
            }
            let parts = dict.decomposition(&chars).unwrap_or_default().join("|");
            println!("COMPOUNDONLY {word}\t{parts}");
        }
    }

    /// Prints how the compound checker takes the words of `HARPER_DEBUG_WORDS` apart.
    #[test]
    #[ignore = "reads the words from the environment"]
    fn show_german_decompositions() {
        let Ok(words) = std::env::var("HARPER_DEBUG_WORDS") else {
            return;
        };
        let dict = crate::spell::compound_aware_german_dictionary();
        for word in words.split_whitespace() {
            let chars: Vec<char> = word.chars().collect();
            println!("DECOMP {word}: {:?}", dict.decomposition(&chars));
        }
    }
}
