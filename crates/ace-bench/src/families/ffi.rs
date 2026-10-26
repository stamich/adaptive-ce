//! `ffi` (ACE 0.7.0): what the C ABI costs on top of the Rust facade.
//!
//! * `compress` / `decompress` rows — the mixed 16 MiB workload through `ace::Engine` and through
//!   the exported `ace_*` functions of `ace-ffi` (the same code a JVM or C host calls), each with
//!   its own Harness V3 timing, plus a check that both produce the same bytes.
//! * `small_compress_*` / `small_decompress_*` rows — buffers of 64 B, 1 KiB and 16 KiB, where the
//!   fixed cost of a call (handle checks, `catch_unwind`, one `AceBuffer`) is visible.
//!
//! The JVM side is measured by `FfiBenchmark` (`jvm/ace-native`), on the same operations. The gates
//! (C ABI >= 0.97x of the facade on 16 MiB, JVM >= 0.90x) live in
//! `tools/ace-check_regressions0.8.0.py` (section `ffi`).

use std::ptr;

use ace_ffi::{
    ace_buffer_data, ace_buffer_free, ace_buffer_len, ace_compress, ace_decompress,
    ace_engine_free, ace_engine_new, AceBuffer, AceEngine as CEngine, ACE_OK,
};

use crate::prelude::*;

/// Size of the large workload.
const LARGE_MIB: usize = 16;
/// Sizes of the call-overhead rows.
const SMALL_SIZES: [usize; 3] = [64, 1024, 16 * 1024];

/// An engine created through the C ABI, released on drop.
struct CAbi {
    handle: *mut CEngine,
}

impl CAbi {
    /// `ace_engine_new` with the default configuration.
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut handle = ptr::null_mut();
        // SAFETY: null config and limits select the defaults; `handle` is writable.
        let status = unsafe { ace_engine_new(ptr::null(), ptr::null(), &mut handle) };
        if status != ACE_OK || handle.is_null() {
            return Err(format!("ace_engine_new failed with status {status}").into());
        }
        Ok(Self { handle })
    }

    /// Reads the length and address of an `AceBuffer` and releases it. The bytes are not copied:
    /// a host that wraps the buffer (Panama `reinterpret`) does not copy either, and copying
    /// 16 MiB would measure `memcpy`, not the ABI.
    fn release(buffer: *mut AceBuffer) -> usize {
        // SAFETY: `buffer` is a live handle returned by ACE, freed exactly once.
        unsafe {
            let length = ace_buffer_len(buffer);
            black_box(ace_buffer_data(buffer));
            ace_buffer_free(buffer);
            length
        }
    }

    /// Copies an `AceBuffer` into a `Vec` and releases it (used once, to compare the bytes).
    fn take(buffer: *mut AceBuffer) -> Vec<u8> {
        // SAFETY: `buffer` is a live handle returned by ACE, freed exactly once after the copy.
        unsafe {
            let bytes = std::slice::from_raw_parts(ace_buffer_data(buffer), ace_buffer_len(buffer))
                .to_vec();
            ace_buffer_free(buffer);
            bytes
        }
    }

    /// `ace_compress` without statistics; the output stays in the buffer until released.
    fn compress_raw(&self, input: &[u8]) -> Result<*mut AceBuffer, Box<dyn std::error::Error>> {
        let mut out = ptr::null_mut();
        // SAFETY: the engine is live, `input` is valid for its length, the outputs are writable.
        let status = unsafe {
            ace_compress(
                self.handle,
                input.as_ptr(),
                input.len(),
                &mut out,
                ptr::null_mut(),
            )
        };
        if status != ACE_OK {
            return Err(format!("ace_compress failed with status {status}").into());
        }
        Ok(out)
    }

    /// `ace_compress`, then the length of the output (no copy).
    fn compress(&self, input: &[u8]) -> Result<usize, Box<dyn std::error::Error>> {
        Ok(Self::release(self.compress_raw(input)?))
    }

    /// `ace_compress`, then a copy of the output.
    fn compress_copy(&self, input: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        Ok(Self::take(self.compress_raw(input)?))
    }

    /// `ace_decompress`, then the length of the output (no copy).
    fn decompress(&self, input: &[u8]) -> Result<usize, Box<dyn std::error::Error>> {
        Ok(Self::release(self.decompress_raw(input)?))
    }

    /// `ace_decompress`, then a copy of the output.
    fn decompress_copy(&self, input: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        Ok(Self::take(self.decompress_raw(input)?))
    }

    /// `ace_decompress`; the output stays in the buffer until released.
    fn decompress_raw(&self, input: &[u8]) -> Result<*mut AceBuffer, Box<dyn std::error::Error>> {
        let mut out = ptr::null_mut();
        // SAFETY: the engine is live, `input` is valid for its length, the output is writable.
        let status = unsafe { ace_decompress(self.handle, input.as_ptr(), input.len(), &mut out) };
        if status != ACE_OK {
            return Err(format!("ace_decompress failed with status {status}").into());
        }
        Ok(out)
    }
}

impl Drop for CAbi {
    fn drop(&mut self) {
        // SAFETY: the handle came from `ace_engine_new` and is released once.
        unsafe { ace_engine_free(self.handle) };
    }
}

/// One row: the same operation through the facade and through the C ABI.
fn row(
    path: &str,
    workload_id: &str,
    bytes: usize,
    facade: &SampleStats,
    c_abi: &SampleStats,
    identical: bool,
) -> Value {
    let mut row = JsonObjectBuilder::new();
    row.field("workload_id", workload_id)
        .field("path", path)
        .field("bytes_processed", bytes)
        .field("identical_output", identical)
        .value("timing_facade", timing_json(facade, bytes))
        .value("timing_c_abi", timing_json(c_abi, bytes));
    row.build()
}

/// Measures compress and decompress of `data` through both APIs; `tag` names the rows.
fn measure_pair(
    tag: &str,
    workload_id: &str,
    data: &[u8],
    facade: &ace::Engine,
    c_abi: &CAbi,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let (facade_compress, via_facade) = measure(|| Ok(facade.compress(data)?))?;
    let (c_compress, _) = measure(|| c_abi.compress(data))?;
    let same_bytes = via_facade == c_abi.compress_copy(data)?;

    let (facade_decompress, restored_facade) = measure(|| Ok(facade.decompress(&via_facade)?))?;
    let (c_decompress, _) = measure(|| c_abi.decompress(&via_facade))?;
    let same_restored = restored_facade == data && c_abi.decompress_copy(&via_facade)? == data;

    Ok(vec![
        row(
            &format!("{tag}compress"),
            workload_id,
            data.len(),
            &facade_compress,
            &c_compress,
            same_bytes,
        ),
        row(
            &format!("{tag}decompress"),
            workload_id,
            data.len(),
            &facade_decompress,
            &c_decompress,
            same_restored,
        ),
    ])
}

/// Runs the `ffi` family.
pub(crate) fn ffi_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let facade = ace::Engine::new(ace::Config::default())?;
    let c_abi = CAbi::new()?;
    let mixed = mixed_data(LARGE_MIB);

    let mut rows = measure_pair("", "mixed_16m", &mixed, &facade, &c_abi)?;
    for size in SMALL_SIZES {
        let small = &mixed[..size];
        rows.extend(measure_pair(
            "small_",
            &format!("mixed_{size}b"),
            small,
            &facade,
            &c_abi,
        )?);
    }
    Ok(rows)
}
