//! Generator implementations (one function per workload).
//!
//! The byte streams are identical to the generators used by ACE 0.4.x benchmarks, so results
//! stay comparable across milestones.

use crate::{XorShift32, XorShift64};

/// Bytes per MiB.
const MIB: usize = 1024 * 1024;

/// Emits fixed-width little-endian lanes produced by `next(lane_index)` until `bytes` bytes
/// exist, then truncates the final partial lane.
fn lanes<const N: usize>(bytes: usize, mut next: impl FnMut(usize) -> [u8; N]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes + N);
    let mut index = 0usize;
    while out.len() < bytes {
        out.extend_from_slice(&next(index));
        index += 1;
    }
    out.truncate(bytes);
    out
}

/// Repeats `pattern` until `bytes` bytes exist.
fn repeat_pattern(pattern: &[u8], bytes: usize) -> Vec<u8> {
    pattern.iter().copied().cycle().take(bytes).collect()
}

/// `0,1,2,3,0,1,…`.
pub(crate) fn low_cardinality(bytes: usize) -> Vec<u8> {
    repeat_pattern(&[0, 1, 2, 3], bytes)
}

/// 4 KiB runs, byte value advancing by 17 per run.
pub(crate) fn runs(bytes: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes);
    let mut value = 0u8;
    while out.len() < bytes {
        let run = (bytes - out.len()).min(4096);
        out.resize(out.len() + run, value);
        value = value.wrapping_add(17);
    }
    out
}

/// u32 counter starting after 10 000 with step 3.
pub(crate) fn u32_counter(bytes: usize) -> Vec<u8> {
    let mut value = 10_000u32;
    lanes(bytes, |_| {
        value = value.wrapping_add(3);
        value.to_le_bytes()
    })
}

/// u64 series whose step is `(byte offset % 7) + 1`.
pub(crate) fn delta_series(bytes: usize) -> Vec<u8> {
    let mut value = 1_000_000u64;
    lanes(bytes, |index| {
        value = value.wrapping_add(((index * 8) as u64 % 7) + 1);
        value.to_le_bytes()
    })
}

/// Repeated JSON record (newline-terminated).
pub(crate) fn structured_json(bytes: usize) -> Vec<u8> {
    repeat_pattern(
        br#"{"service":"ace","status":"ACTIVE","region":"eu","value":123456}
"#,
        bytes,
    )
}

/// Low byte of a 32-bit xorshift stream.
pub(crate) fn random(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift32::new(0x9E37_79B9);
    (0..bytes).map(|_| rng.next_u32() as u8).collect()
}

/// Four quarters: zeros, u32 ramp (`offset / 16`), JSON lines, random bytes.
///
/// The layout is defined per whole MiB (historical behaviour), then truncated to `bytes`.
pub(crate) fn mixed(bytes: usize) -> Vec<u8> {
    let target = bytes.div_ceil(MIB) * MIB;
    let quarter = target / 4;
    let mut data = vec![0u8; quarter];
    while data.len() < quarter * 2 {
        let value = (data.len() as u32 / 16).to_le_bytes();
        data.extend_from_slice(&value);
    }
    while data.len() < quarter * 3 {
        data.extend_from_slice(
            b"{\"status\":\"ACTIVE\",\"service\":\"graphnet\",\"region\":\"eu\"}\n",
        );
    }
    let mut rng = XorShift32::new(0x9E37_79B9);
    while data.len() < target {
        data.push(rng.next_u32() as u8);
    }
    data.truncate(bytes);
    data
}

/// u64 timestamps with step `1000 + (i % 3)`.
pub(crate) fn u64_timestamps_ms(bytes: usize) -> Vec<u8> {
    let mut value = 1_780_000_000_000u64;
    lanes(bytes, |index| {
        value = value.wrapping_add(1000 + (index as u64 % 3));
        value.to_le_bytes()
    })
}

/// u64 values with constant step 1000.
pub(crate) fn u64_fixed_step(bytes: usize) -> Vec<u8> {
    let mut value = 1_780_000_000_000u64;
    lanes(bytes, |_| {
        value = value.wrapping_add(1000);
        value.to_le_bytes()
    })
}

/// u64 nanosecond clock: step 1 ms plus up to 1 ms of jitter.
pub(crate) fn u64_timestamps_ns(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift64::new(0x9E37_79B9_7F4A_7C15);
    let mut value = 1_700_000_000_000_000_000u64;
    lanes(bytes, |_| {
        value += 1_000_000 + rng.next_u64() % 1_000_000;
        value.to_le_bytes()
    })
}

/// u32 sawtooth `100000 + (i % 4096)`.
pub(crate) fn gauge_sawtooth(bytes: usize) -> Vec<u8> {
    lanes(bytes, |index| {
        (100_000u32 + (index as u32 % 4096)).to_le_bytes()
    })
}

/// u32 counter with step 1 and a 100 000 jump every 1024 values.
pub(crate) fn monotonic_outliers(bytes: usize) -> Vec<u8> {
    let mut value = 1_000_000u32;
    lanes(bytes, |index| {
        value = value.wrapping_add(if index % 1024 == 0 { 100_000 } else { 1 });
        value.to_le_bytes()
    })
}

/// u32 counter with step `(i % 17) + 1`.
pub(crate) fn delta_variable(bytes: usize) -> Vec<u8> {
    let mut value = 100_000u32;
    lanes(bytes, |index| {
        value = value.wrapping_add((index as u32 % 17) + 1);
        value.to_le_bytes()
    })
}

/// u16 samples starting at 65 000 with random steps 0..=6 (wraps around 65 535).
pub(crate) fn u16_wrapping(bytes: usize) -> Vec<u8> {
    let mut rng = XorShift64::new(7);
    let mut value = 65_000u16;
    lanes(bytes, |_| {
        value = value.wrapping_add((rng.next_u64() % 7) as u16);
        value.to_le_bytes()
    })
}
