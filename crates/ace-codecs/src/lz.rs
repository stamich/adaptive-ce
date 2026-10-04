use ace_core::{AceError, AceResult, LzMode};

const MIN_MATCH: usize = 4;
const MAX_MATCH: usize = 130;
const WINDOW: usize = 65_535;
const HASH_BITS: usize = 16;
const HASH_SIZE: usize = 1 << HASH_BITS;
const MAX_CHAIN: usize = 16;

/// Encodes a byte slice into a simple LZ token stream.
///
/// Token format: `0LLLLLLL <literal bytes>` for a literal packet and
/// `1LLLLLLL <distance:u16-le>` for a match of `L + MIN_MATCH` bytes.
pub fn lz_encode(input: &[u8], mode: LzMode) -> Vec<u8> {
    if input.is_empty() {
        return Vec::new();
    }
    let mut head = vec![usize::MAX; HASH_SIZE];
    let mut prev = vec![usize::MAX; input.len()];
    let mut out = Vec::with_capacity(input.len());
    let mut literal_start = 0usize;
    let mut i = 0usize;
    while i < input.len() {
        let best = if i + MIN_MATCH <= input.len() {
            let h = hash4(&input[i..i + 4]);
            let candidate = head[h];
            prev[i] = candidate;
            head[h] = i;
            find_best(input, i, candidate, &prev, mode)
        } else {
            None
        };
        if let Some((distance, len)) = best {
            flush_literals(&mut out, &input[literal_start..i]);
            let encoded_len = len - MIN_MATCH;
            out.push(0x80 | encoded_len as u8);
            out.extend_from_slice(&(distance as u16).to_le_bytes());
            for p in i + 1..i + len {
                if p + MIN_MATCH <= input.len() {
                    let h = hash4(&input[p..p + 4]);
                    prev[p] = head[h];
                    head[h] = p;
                }
            }
            i += len;
            literal_start = i;
        } else {
            i += 1;
        }
    }
    flush_literals(&mut out, &input[literal_start..]);
    out
}

/// Decodes a simple LZ token stream with strict reference and output validation.
pub fn lz_decode(input: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    let mut out = Vec::with_capacity(expected_size.min(1024 * 1024));
    let mut i = 0usize;
    while i < input.len() {
        let control = input[i];
        i += 1;
        if control & 0x80 == 0 {
            let len = (control as usize) + 1;
            let end = i
                .checked_add(len)
                .ok_or(AceError::Malformed("LZ literal overflow"))?;
            let lit = input
                .get(i..end)
                .ok_or(AceError::Malformed("truncated LZ literal"))?;
            if out.len() + len > expected_size {
                return Err(AceError::Malformed("LZ literal exceeds expected output"));
            }
            out.extend_from_slice(lit);
            i = end;
        } else {
            let len = ((control & 0x7f) as usize) + MIN_MATCH;
            let dist_bytes = input
                .get(i..i + 2)
                .ok_or(AceError::Malformed("truncated LZ distance"))?;
            i += 2;
            let distance = u16::from_le_bytes([dist_bytes[0], dist_bytes[1]]) as usize;
            if distance == 0 || distance > out.len() || distance > WINDOW {
                return Err(AceError::InvalidLzReference);
            }
            if out.len() + len > expected_size {
                return Err(AceError::Malformed("LZ match exceeds expected output"));
            }
            for _ in 0..len {
                let b = out[out.len() - distance];
                out.push(b);
            }
        }
    }
    if out.len() != expected_size {
        return Err(AceError::Malformed("LZ decoded size mismatch"));
    }
    Ok(out)
}

/// Computes a 16-bit hash bucket from four consecutive source bytes.
fn hash4(bytes: &[u8]) -> usize {
    let v = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    ((v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize) & (HASH_SIZE - 1)
}

/// Searches either one candidate or a bounded previous-position chain for the longest valid match.
fn find_best(
    input: &[u8],
    pos: usize,
    mut candidate: usize,
    prev: &[usize],
    mode: LzMode,
) -> Option<(usize, usize)> {
    let max_steps = match mode {
        LzMode::Fast => 1,
        LzMode::Balanced => MAX_CHAIN,
    };
    let mut best = None;
    let mut best_len = 0usize;
    for _ in 0..max_steps {
        if candidate == usize::MAX || candidate >= pos {
            break;
        }
        let distance = pos - candidate;
        if distance > WINDOW {
            candidate = prev[candidate];
            continue;
        }
        let max = MAX_MATCH.min(input.len() - pos);
        let mut len = 0usize;
        while len < max && input[candidate + len] == input[pos + len] {
            len += 1;
        }
        if len >= MIN_MATCH && len > best_len {
            best_len = len;
            best = Some((distance, len));
        }
        candidate = prev[candidate];
    }
    best
}

/// Emits literal data in packets that fit the 7-bit packet length field.
fn flush_literals(out: &mut Vec<u8>, literals: &[u8]) {
    let mut start = 0usize;
    while start < literals.len() {
        let len = (literals.len() - start).min(128);
        out.push((len - 1) as u8);
        out.extend_from_slice(&literals[start..start + len]);
        start += len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies overlapping LZ references are decoded correctly.
    #[test]
    fn repeated_data_roundtrip() {
        let data = b"abcabcabcabcabcabcabcabc";
        for mode in [LzMode::Fast, LzMode::Balanced] {
            let e = lz_encode(data, mode);
            assert_eq!(lz_decode(&e, data.len()).unwrap(), data);
        }
    }
}
