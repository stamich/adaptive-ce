//! Tests of `ace_bench` `families` (formerly the inline unit tests of `src`).

use ace_bench::{select_families, FAMILIES};

#[test]
fn family_names_are_unique() {
    let mut names: Vec<&str> = FAMILIES.iter().map(|(name, _)| *name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), FAMILIES.len());
}

#[test]
fn unknown_family_is_rejected() {
    assert!(select_families("no-such-family").is_err());
    assert_eq!(select_families("all").map(|f| f.len()), Ok(FAMILIES.len()));
}
