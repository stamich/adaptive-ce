//! `include/ace.h` declares exactly the functions the crate exports.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Names of the `#[no_mangle]` functions in `src/`.
fn exported() -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(crate_dir().join("src")).unwrap() {
        let text = fs::read_to_string(entry.unwrap().path()).unwrap();
        let mut marked = false;
        for line in text.lines() {
            let line = line.trim();
            if line == "#[no_mangle]" {
                marked = true;
            } else if marked {
                let name = line
                    .split("fn ")
                    .nth(1)
                    .and_then(|rest| rest.split('(').next())
                    .unwrap();
                names.insert(name.trim().to_owned());
                marked = false;
            }
        }
    }
    names
}

fn header() -> String {
    fs::read_to_string(crate_dir().join("../../include/ace.h")).unwrap()
}

#[test]
fn every_exported_function_is_declared() {
    let header = header();
    let names = exported();
    assert!(names.len() >= 26, "found only {names:?}");
    for name in &names {
        assert!(
            header.contains(&format!(" {name}(")) || header.contains(&format!("*{name}(")),
            "{name} is missing from include/ace.h"
        );
    }
}

#[test]
fn the_header_declares_nothing_else() {
    let names = exported();
    for line in header()
        .lines()
        .filter(|line| line.contains("ace_") && line.ends_with(");"))
    {
        let declared = line
            .split('(')
            .next()
            .unwrap()
            .rsplit(['*', ' '])
            .next()
            .unwrap();
        assert!(
            names.contains(declared),
            "{declared} is declared but not exported"
        );
    }
}

#[test]
fn the_header_carries_the_abi_version() {
    assert!(header().contains(&format!(
        "#define ACE_ABI_VERSION {}",
        ace_ffi::ACE_ABI_VERSION
    )));
}
