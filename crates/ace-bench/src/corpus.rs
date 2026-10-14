//! Benchmark-side adapters over the shared `ace-corpus` generators.

use std::collections::BTreeMap;
use std::sync::Mutex;

use ace_corpus::Workload;

use crate::json::sha256_hex;

/// Bytes per MiB.
const MIB: usize = 1024 * 1024;

/// `"<workload>@<bytes>"` → SHA-256 of every input generated since the last
/// [`take_workload_fingerprints`] (schema 2.2 `workload_fingerprints`).
static FINGERPRINTS: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

/// Records the SHA-256 of `data` under `name@len` (hashed once per key and family) and
/// returns `data`. Called during setup, never inside a timed region.
pub(crate) fn fingerprinted(name: &str, data: Vec<u8>) -> Vec<u8> {
    let key = format!("{name}@{}", data.len());
    let mut fingerprints = FINGERPRINTS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    fingerprints.entry(key).or_insert_with(|| sha256_hex(&data));
    data
}

/// Returns and clears the fingerprints recorded for the family that just ran.
pub(crate) fn take_workload_fingerprints() -> BTreeMap<String, String> {
    let mut fingerprints = FINGERPRINTS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    std::mem::take(&mut *fingerprints)
}

/// `bytes` bytes of `workload`, fingerprinted.
pub(crate) fn generate(workload: Workload, bytes: usize) -> Vec<u8> {
    fingerprinted(workload.name(), workload.generate(bytes))
}

/// The heterogeneous `mixed` corpus of `mebibytes` MiB used across benchmark families.
pub(crate) fn mixed_data(mebibytes: usize) -> Vec<u8> {
    generate(Workload::Mixed, mebibytes * MIB)
}

/// Generates `bytes` bytes of the named workload (benchmark ids and historical aliases).
pub(crate) fn workload_bytes(name: &str, bytes: usize) -> Vec<u8> {
    match Workload::from_name(name) {
        Some(workload) => generate(workload, bytes),
        None => panic!("unknown benchmark workload: {name}"),
    }
}

/// Returns a stable coarse data-class label for one quarter of the synthetic mixed corpus.
pub(crate) fn data_class(block_index: usize, block_count: usize) -> &'static str {
    let quarter = (block_count / 4).max(1);
    match block_index / quarter {
        0 => "zeros",
        1 => "numeric",
        2 => "structured",
        _ => "random",
    }
}
