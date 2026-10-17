#![no_main]

use ace_codecs::{ts1_decode, ts1_encode_with, ts1_encoded_len, TimeSeriesLayout};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Decimal-shaped series: 4-byte groups are `digits / 10^scale` (what ALP is built for), with
    // arbitrary bit patterns mixed in so exceptions (NaN, -0.0, Inf, noise) occur as well.
    let Some((&scale, rest)) = data.split_first() else {
        return;
    };
    let scale = i32::from(scale % 10);
    let mut f64s = Vec::new();
    let mut f32s = Vec::new();
    for (index, chunk) in rest.chunks_exact(4).enumerate() {
        let digits = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        if index % 11 == 5 {
            f64s.extend_from_slice(&u64::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3], chunk[3], chunk[2], chunk[1], chunk[0]]).to_le_bytes());
            f32s.extend_from_slice(&chunk[..4]);
        } else {
            f64s.extend_from_slice(&(f64::from(digits) / 10f64.powi(scale)).to_le_bytes());
            f32s.extend_from_slice(&((digits % 1_000_000) as f32 / 10f32.powi(scale.min(6))).to_le_bytes());
        }
    }
    for (bytes, layout) in [
        (&f64s, TimeSeriesLayout::ALP_F64),
        (&f32s, TimeSeriesLayout::ALP_F32),
        (&rest.to_vec(), TimeSeriesLayout::ALP_F64),
        (&rest.to_vec(), TimeSeriesLayout::ALP_F32),
    ] {
        let encoded = ts1_encode_with(bytes, layout).expect("valid layout");
        assert_eq!(ts1_encoded_len(bytes, layout).expect("valid layout"), encoded.len());
        assert_eq!(ts1_decode(&encoded, bytes.len()).expect("own payload decodes"), *bytes);
    }
});
