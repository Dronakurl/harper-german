//! Turns the `"morphology"` objects of `annotations.json` into [`LanguageBits`].
//!
//! `annotations.json` describes a form's features readably:
//!
//! ```json
//! "metadata": { "noun": {}, "morphology": { "noun": { "gender": "Masculine" } } }
//! ```
//!
//! Harper's annotation parser only knows `"language_bits"`, and it skips keys it
//! does not know without a word. So every `"morphology"` object is rewritten
//! into its bits before the parser sees the file, and anything this cannot
//! read is an error rather than a feature that silently goes missing.

use harper_core::LanguageBits;
use serde_json::{Map, Value};

use super::{Morphology, bits};

/// `annotations` with every `"morphology"` object replaced by its `"language_bits"`.
pub fn prepare(annotations: &str) -> Result<String, String> {
    let mut value: Value = serde_json::from_str(annotations).map_err(|e| e.to_string())?;
    rewrite(&mut value)?;
    Ok(value.to_string())
}

fn rewrite(value: &mut Value) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            if let Some(morphology) = object.remove("morphology") {
                replace(object, morphology)?;
            }
            object.values_mut().try_for_each(rewrite)
        }
        Value::Array(items) => items.iter_mut().try_for_each(rewrite),
        _ => Ok(()),
    }
}

fn replace(object: &mut Map<String, Value>, morphology: Value) -> Result<(), String> {
    let text = morphology.to_string();
    let morphology: Morphology =
        serde_json::from_value(morphology).map_err(|e| format!("morphology {text}: {e}"))?;
    let bits = bits::encode(&morphology);
    if bits.is_empty() {
        return Err(format!("morphology {text} says nothing"));
    }
    if object.contains_key("language_bits") {
        return Err(format!("morphology {text} next to language_bits"));
    }
    let LanguageBits([low, high]) = bits;
    object.insert("language_bits".to_owned(), Value::from(vec![low, high]));
    Ok(())
}

#[cfg(test)]
mod tests {
    use harper_core::DictWordMetadata;

    use super::super::{Gender, MorphologyExt};
    use super::prepare;

    #[test]
    fn a_morphology_object_becomes_bits_harper_reads() {
        let json = prepare(r#"{"noun":{},"morphology":{"noun":{"gender":"Masculine"}}}"#).unwrap();
        assert!(!json.contains("morphology"), "{json}");

        let metadata: DictWordMetadata = serde_json::from_str(&json).unwrap();
        assert!(metadata.noun.is_some());
        assert_eq!(metadata.get_noun_gender(), Some(Gender::Masculine));
    }

    #[test]
    fn nested_objects_are_rewritten_too() {
        let json = prepare(
            r#"{"A":{"target":[{"metadata":{"morphology":{"verb":{"person":"First"}}}}]}}"#,
        )
        .unwrap();
        assert!(json.contains("language_bits") && !json.contains("morphology"));
    }

    /// Harper would skip what it cannot read, so this must not.
    #[test]
    fn what_cannot_be_read_is_an_error() {
        assert!(prepare(r#"{"morphology":{"adjective":{}}}"#).is_err());
        assert!(prepare(r#"{"morphology":{"noun":{"degree":"Comparative"}}}"#).is_err());
        assert!(prepare(r#"{"morphology":{"noun":{"gender":"Animate"}}}"#).is_err());
        assert!(prepare(r#"{"morphology":{}}"#).is_err());
    }

    /// Every `"morphology"` in the real file is understood and carries something.
    #[test]
    fn the_german_annotations_are_all_rewritten() {
        let json = prepare(include_str!("../../data/annotations.json")).unwrap();
        assert!(!json.contains("\"morphology\""));
        assert!(json.contains("language_bits"));
    }
}
