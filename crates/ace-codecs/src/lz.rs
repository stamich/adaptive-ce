use ace_core::{AceError, AceResult, LzMode};
use ace_simd::common_prefix_len;

/// Shortest match (bytes) worth encoding as a reference.
const MIN_MATCH: usize = 4;
/// Longest match (bytes) one reference token can express (`MIN_MATCH + 126`).
const MAX_MATCH: usize = 130;
/// Largest back-reference distance, bounded by the 16-bit distance field.
const WINDOW: usize = 65_535;
/// Number of bits of the 4-byte rolling hash.
const HASH_BITS: usize = 16;
/// Number of hash buckets (`1 << HASH_BITS`).
const HASH_SIZE: usize = 1 << HASH_BITS;
/// Maximum candidates inspected per position in `LzMode::Balanced`.
const MAX_CHAIN: usize = 16;

/// Largest input (bytes) encoded with the per-thread tables. Longer inputs allocate their own:
/// the cost of clearing the tables is then small next to the encoding, and the plain loop is
/// about 7 % faster than the one that tracks the touched buckets.
pub const LZ_SCRATCH_MAX_INPUT: usize = 64 * 1024;

/// Hash-chain tables reused by the calls of one thread.
///
/// The planner trial-encodes dozens of small samples per block; allocating and clearing a
/// 512 KiB hash table and an 8-byte-per-input-byte chain table for each of them was the largest
/// source of allocation churn of the encoder. `head` is all `usize::MAX` between calls and only
/// the buckets a call touched are reset; `prev` is never cleared, because the chain walk only
/// reads entries of positions inserted by the same call.
struct LzScratch {
    /// Most recent position of each hash bucket; `usize::MAX` when empty.
    head: Vec<usize>,
    /// Previous position with the same bucket, indexed by position.
    prev: Vec<usize>,
    /// Buckets written by the current call.
    touched: Vec<u32>,
    /// `true` while a call is running; a call that unwound leaves the tables dirty.
    in_use: bool,
}

impl LzScratch {
    const fn new() -> Self {
        Self {
            head: Vec::new(),
            prev: Vec::new(),
            touched: Vec::new(),
            in_use: false,
        }
    }

    /// Makes `head` all-empty and `prev` at least `len` entries long.
    fn prepare(&mut self, len: usize) {
        if self.in_use || self.head.len() != HASH_SIZE {
            self.head.clear();
            self.head.resize(HASH_SIZE, usize::MAX);
            self.touched.clear();
        }
        if self.prev.len() < len {
            self.prev.resize(len, usize::MAX);
        }
        self.in_use = true;
    }

    /// Empties the buckets this call touched.
    fn finish(&mut self) {
        for &bucket in &self.touched {
            self.head[bucket as usize] = usize::MAX;
        }
        self.touched.clear();
        self.in_use = false;
    }

    /// Records `position` as the newest entry of `bucket` and returns the previous newest.
    #[inline]
    fn insert(&mut self, bucket: usize, position: usize) -> usize {
        let older = self.head[bucket];
        if older == usize::MAX {
            // `bucket < HASH_SIZE = 1 << 16`.
            self.touched.push(bucket as u32);
        }
        self.prev[position] = older;
        self.head[bucket] = position;
        older
    }
}

thread_local! {
    /// The hash-chain tables of the calling thread.
    static LZ_SCRATCH: std::cell::RefCell<LzScratch> = const { std::cell::RefCell::new(LzScratch::new()) };
}

/// Test hook: leaves this thread's hash tables dirty, exactly as an encode that unwound would.
///
/// The next [`lz_encode`] call on the thread must repair the tables and still produce the bytes
/// of [`lz_encode_allocating`].
#[doc(hidden)]
pub fn lz_leave_scratch_dirty() {
    LZ_SCRATCH.with(|cell| {
        let mut scratch = cell.borrow_mut();
        scratch.prepare(100);
        scratch.insert(5, 3);
        scratch.insert(77, 9);
    });
}

/// Encodes a byte slice into a simple LZ token stream.
///
/// Token format: `0LLLLLLL <literal bytes>` for a literal packet and
/// `1LLLLLLL <distance:u16-le>` for a match of `L + MIN_MATCH` bytes.
pub fn lz_encode(input: &[u8], mode: LzMode) -> Vec<u8> {
    if input.is_empty() {
        return Vec::new();
    }
    if input.len() > LZ_SCRATCH_MAX_INPUT {
        return lz_encode_allocating(input, mode);
    }
    LZ_SCRATCH.with(|cell| match cell.try_borrow_mut() {
        Ok(mut scratch) => {
            scratch.prepare(input.len());
            let out = lz_encode_with(input, mode, &mut scratch);
            scratch.finish();
            out
        }
        // Re-entrant call (not expected): use private tables.
        Err(_) => {
            let mut scratch = LzScratch::new();
            scratch.prepare(input.len());
            lz_encode_with(input, mode, &mut scratch)
        }
    })
}

/// The encoder with hash tables allocated for this call (the path of inputs above
/// [`LZ_SCRATCH_MAX_INPUT`]).
///
/// Produces exactly the bytes of [`lz_encode`] for every input; it is the reference the reuse of
/// the per-thread tables is tested against.
pub fn lz_encode_allocating(input: &[u8], mode: LzMode) -> Vec<u8> {
    lz_encode_from(input, 0, mode)
}

/// Encodes `input` with `dictionary` as history that precedes it (Format 1.6).
///
/// The token stream is the ordinary one: a match may point back into the last 65 535 bytes of
/// `dictionary ‖ output`, so the first bytes of the input can already be matches against the
/// dictionary. Decode with [`lz_decode_with_dictionary`] and the same dictionary. An empty
/// dictionary gives exactly the bytes of [`lz_encode`].
pub fn lz_encode_with_dictionary(input: &[u8], dictionary: &[u8], mode: LzMode) -> Vec<u8> {
    if dictionary.is_empty() {
        return lz_encode(input, mode);
    }
    if input.is_empty() {
        return Vec::new();
    }
    let mut history = Vec::with_capacity(dictionary.len() + input.len());
    history.extend_from_slice(dictionary);
    history.extend_from_slice(input);
    lz_encode_from(&history, dictionary.len(), mode)
}

/// Encodes `data[start..]`; `data[..start]` is history that is indexed but not emitted.
///
/// With `start == 0` this is the plain encoder. The tables are allocated for the call.
fn lz_encode_from(data: &[u8], start: usize, mode: LzMode) -> Vec<u8> {
    let mut head = vec![usize::MAX; HASH_SIZE];
    let mut prev = vec![usize::MAX; data.len()];
    // Index the history so that matches can start in it. A position whose four bytes run into
    // the input is fine: the bytes exist in `data` and in the decoder's output buffer.
    for p in 0..start {
        if p + MIN_MATCH <= data.len() {
            let h = hash4(&data[p..p + 4]);
            prev[p] = head[h];
            head[h] = p;
        }
    }
    let mut out = Vec::with_capacity(data.len() - start);
    let mut literal_start = start;
    let mut i = start;
    while i < data.len() {
        let best = if i + MIN_MATCH <= data.len() {
            let h = hash4(&data[i..i + 4]);
            let candidate = head[h];
            prev[i] = candidate;
            head[h] = i;
            find_best(data, i, candidate, &prev, mode)
        } else {
            None
        };
        if let Some((distance, len)) = best {
            flush_literals(&mut out, &data[literal_start..i]);
            let encoded_len = len - MIN_MATCH;
            out.push(0x80 | encoded_len as u8);
            out.extend_from_slice(&(distance as u16).to_le_bytes());
            for p in i + 1..i + len {
                if p + MIN_MATCH <= data.len() {
                    let h = hash4(&data[p..p + 4]);
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
    flush_literals(&mut out, &data[literal_start..]);
    out
}

/// The encoder proper, over prepared tables.
fn lz_encode_with(input: &[u8], mode: LzMode, scratch: &mut LzScratch) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut literal_start = 0usize;
    let mut i = 0usize;
    while i < input.len() {
        let best = if i + MIN_MATCH <= input.len() {
            let h = hash4(&input[i..i + 4]);
            let candidate = scratch.insert(h, i);
            find_best(input, i, candidate, &scratch.prev, mode)
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
                    scratch.insert(h, p);
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
///
/// # Errors
///
/// [`AceError::Malformed`] for truncated tokens or output that does not end at exactly
/// `expected_size` bytes; [`AceError::InvalidLzReference`] for a zero or out-of-range match
/// distance.
pub fn lz_decode(input: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    lz_decode_after(input, &[], expected_size)
}

/// Decodes a token stream that was encoded with `dictionary` as preset history
/// ([`lz_encode_with_dictionary`]).
///
/// The decoder starts with the dictionary as the first bytes of its output buffer, so a
/// reference may reach back into it, and removes it at the end. A reference that would reach
/// before the start of the dictionary, or farther than 65 535 bytes, is invalid. An empty
/// dictionary is [`lz_decode`].
///
/// # Errors
///
/// The errors of [`lz_decode`]; the distance check counts the dictionary as output.
pub fn lz_decode_with_dictionary(
    input: &[u8],
    dictionary: &[u8],
    expected_size: usize,
) -> AceResult<Vec<u8>> {
    lz_decode_after(input, dictionary, expected_size)
}

/// The decoder: `history` is the (possibly empty) dictionary the output continues.
fn lz_decode_after(input: &[u8], history: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    let base = history.len();
    let limit = base
        .checked_add(expected_size)
        .ok_or(AceError::Malformed("LZ expected size overflow"))?;
    let mut out = Vec::with_capacity(limit.min(1024 * 1024).max(base));
    out.extend_from_slice(history);
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
            if out.len() + len > limit {
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
            if out.len() + len > limit {
                return Err(AceError::Malformed("LZ match exceeds expected output"));
            }
            copy_match(&mut out, distance, len);
        }
    }
    if out.len() != limit {
        return Err(AceError::Malformed("LZ decoded size mismatch"));
    }
    if base > 0 {
        out.drain(..base);
    }
    Ok(out)
}

/// Appends `len` bytes copied from `distance` bytes back, honouring overlap (LZ77 semantics).
///
/// When `distance >= len` the source range is fully initialised, so one `extend_from_within`
/// (a `memcpy`) suffices. Overlapping matches (`distance < len`, e.g. run-length patterns)
/// repeat the `distance`-byte period chunk by chunk, never reading bytes that have not been
/// written yet. Callers must have validated `0 < distance <= out.len()`.
#[inline]
fn copy_match(out: &mut Vec<u8>, distance: usize, len: usize) {
    let start = out.len() - distance;
    if distance >= len {
        out.extend_from_within(start..start + len);
    } else {
        let mut remaining = len;
        while remaining > 0 {
            // Each pass copies at most one full period, all of which already exists.
            let chunk = remaining.min(distance);
            out.extend_from_within(start..start + chunk);
            remaining -= chunk;
        }
    }
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
        let len = common_prefix_len(&input[candidate..candidate + max], &input[pos..pos + max]);
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
