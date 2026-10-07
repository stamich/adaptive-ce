//! Benchmark-side adapters over the shared `ace-corpus` generators.

use ace_corpus::Workload;

/// Bytes per MiB.
const MIB: usize = 1024 * 1024;

/// The heterogeneous `mixed` corpus of `mebibytes` MiB used across benchmark families.
pub(crate) fn mixed_data(mebibytes: usize) -> Vec<u8> {
    Workload::Mixed.generate(mebibytes * MIB)
}

/// Generates `bytes` bytes of the named workload (benchmark ids and historical aliases).
pub(crate) fn workload_bytes(name: &str, bytes: usize) -> Vec<u8> {
    match Workload::from_name(name) {
        Some(workload) => workload.generate(bytes),
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
