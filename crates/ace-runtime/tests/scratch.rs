//! Tests of `ace_runtime` `scratch` (formerly the inline unit tests of `src`).

use ace_runtime::WorkerScratch;

/// Verifies the manual default implementation zeroes histogram storage and starts with empty buffers.
#[test]
fn worker_scratch_default_is_empty_and_zeroed() {
    let scratch = WorkerScratch::default();
    assert!(scratch.transform_buffer.is_empty());
    assert!(scratch.codec_buffer.is_empty());
    assert!(scratch.entropy_buffer.is_empty());
    assert!(scratch.histogram.iter().all(|&value| value == 0));
}
