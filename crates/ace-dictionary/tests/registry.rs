//! Tests of `ace_dictionary` `registry` (formerly the inline unit tests of `src`).

use ace_core::{DictionaryId, DictionaryScope};
use ace_dictionary::*;

/// Insert, lookup, replace and remove behave like a map.
#[test]
fn registry_lifecycle() {
    let mut registry = DictionaryRegistry::new();
    assert!(registry.is_empty());
    let id = DictionaryId(7);
    assert!(registry
        .insert(Dictionary::new(id, DictionaryScope::Block, vec![1, 2, 3]))
        .is_none());
    assert_eq!(registry.get(id).unwrap().unwrap().len(), 3);
    assert!(registry
        .insert(Dictionary::new(id, DictionaryScope::Block, vec![9]))
        .is_some());
    assert_eq!(registry.len(), 1);
    assert!(registry.remove(id).is_some());
    assert!(registry.get(id).unwrap().is_none());
}
