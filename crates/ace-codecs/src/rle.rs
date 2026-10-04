use ace_core::{AceError, AceResult};

const MIN_RUN: usize = 4;
const MAX_PACKET: usize = 128;

/// Encodes bytes using packet RLE with 1..=128-byte literal and repeated-run packets.
pub fn rle_encode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0usize;
    while i < input.len() {
        let run = run_length(input, i).min(MAX_PACKET);
        if run >= MIN_RUN {
            out.push(0x80 | ((run - 1) as u8));
            out.push(input[i]);
            i += run;
            continue;
        }
        let start = i;
        while i < input.len() && i - start < MAX_PACKET {
            let next = run_length(input, i);
            if next >= MIN_RUN {
                break;
            }
            let remaining = MAX_PACKET - (i - start);
            let advance = next.max(1).min(remaining);
            i += advance;
        }
        let len = i - start;
        debug_assert!((1..=MAX_PACKET).contains(&len));
        out.push((len - 1) as u8);
        out.extend_from_slice(&input[start..i]);
    }
    out
}

/// Decodes packet RLE while enforcing the caller-provided output size bound.
pub fn rle_decode(input: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    let mut out = Vec::with_capacity(expected_size.min(1024 * 1024));
    let mut i = 0usize;
    while i < input.len() {
        let control = input[i];
        i += 1;
        let len = ((control & 0x7f) as usize) + 1;
        if out
            .len()
            .checked_add(len)
            .ok_or(AceError::Malformed("RLE output overflow"))?
            > expected_size
        {
            return Err(AceError::Malformed("RLE expands beyond expected size"));
        }
        if control & 0x80 != 0 {
            let &value = input
                .get(i)
                .ok_or(AceError::Malformed("truncated RLE run"))?;
            i += 1;
            out.extend(std::iter::repeat(value).take(len));
        } else {
            let end = i
                .checked_add(len)
                .ok_or(AceError::Malformed("RLE literal overflow"))?;
            let literal = input
                .get(i..end)
                .ok_or(AceError::Malformed("truncated RLE literal"))?;
            out.extend_from_slice(literal);
            i = end;
        }
    }
    if out.len() != expected_size {
        return Err(AceError::Malformed("RLE decoded size mismatch"));
    }
    Ok(out)
}

/// Returns the repeated-byte run length starting at `start`.
fn run_length(input: &[u8], start: usize) -> usize {
    if start >= input.len() {
        return 0;
    }
    let value = input[start];
    let mut i = start + 1;
    while i < input.len() && input[i] == value && i - start < MAX_PACKET {
        i += 1;
    }
    i - start
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies literals, short runs and long runs in the same stream.
    #[test]
    fn mixed_roundtrip() {
        let mut d = b"abc".to_vec();
        d.extend(vec![7u8; 500]);
        d.extend_from_slice(b"xyzxyz");
        assert_eq!(rle_decode(&rle_encode(&d), d.len()).unwrap(), d);
    }

    /// Verifies the literal-packet boundary around the 128-byte wire-format limit.
    #[test]
    fn literal_packet_boundary_roundtrip() {
        for size in [126usize, 127, 128, 129, 130, 255, 256, 257] {
            let data = (0..size)
                .map(|i| ((i * 37 + i / 3) % 251) as u8)
                .collect::<Vec<_>>();
            let encoded = rle_encode(&data);
            let decoded = rle_decode(&encoded, data.len()).unwrap();
            assert_eq!(
                decoded, data,
                "RLE boundary roundtrip failed for size {size}"
            );
        }
    }

    /// Reproduces the ACE 0.3 bug where many three-byte runs could overflow a literal packet.
    #[test]
    fn repeated_short_runs_do_not_overflow_literal_packet() {
        let mut data = Vec::new();
        for value in 0u8..100 {
            data.extend_from_slice(&[value, value, value]);
        }
        let encoded = rle_encode(&data);
        let decoded = rle_decode(&encoded, data.len()).unwrap();
        assert_eq!(decoded, data);
    }

    /// Exercises deterministic pseudo-random payloads across a broad range of lengths.
    #[test]
    fn deterministic_fuzz_style_roundtrip() {
        let mut state = 0x1234_5678u32;
        for size in [0usize, 1, 2, 3, 4, 127, 128, 129, 1024, 4097, 65535] {
            let mut data = Vec::with_capacity(size);
            for _ in 0..size {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                data.push((state >> 24) as u8);
            }
            let encoded = rle_encode(&data);
            let decoded = rle_decode(&encoded, data.len()).unwrap();
            assert_eq!(
                decoded, data,
                "RLE deterministic fuzz-style roundtrip failed for size {size}"
            );
        }
    }
}
