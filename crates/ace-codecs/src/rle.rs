use ace_core::{AceError, AceResult};

/// Shortest run (bytes) that is stored as a run packet instead of literals.
const MIN_RUN: usize = 4;
/// Largest number of bytes one RLE packet can describe.
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
///
/// # Errors
///
/// [`AceError::Malformed`] for a truncated packet or output that does not end at exactly
/// `expected_size` bytes.
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
            out.extend(std::iter::repeat_n(value, len));
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
