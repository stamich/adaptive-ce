//! Canonical code assignment and the two decoding tables built from it.

use ace_core::{AceError, AceResult};

/// Longest canonical code length accepted by encoder and decoder.
pub(crate) const MAX_CODE_LEN: u8 = 32;

/// Bits resolved by one lookup of the primary decoding table.
///
/// 11 bits (2048 entries × 2 bytes) covers every code of typical ACE blocks while staying in L1.
pub(crate) const LOOKUP_BITS: u8 = 11;

/// Derives canonical `(code, length)` pairs from a per-symbol length table.
///
/// Codes are assigned in `(length, symbol)` order. Oversubscribed tables (Kraft sum > 1) are
/// rejected, which guarantees the resulting code is prefix-free.
pub(crate) fn canonical_codes(lengths: &[u8; 256]) -> AceResult<[(u32, u8); 256]> {
    let mut pairs: Vec<(u8, u16)> = lengths
        .iter()
        .enumerate()
        .filter(|(_, &length)| length != 0)
        .map(|(symbol, &length)| (length, symbol as u16))
        .collect();
    if pairs.iter().any(|&(length, _)| length > MAX_CODE_LEN) {
        return Err(AceError::InvalidHuffman("code length too large"));
    }
    pairs.sort_unstable();
    let mut out = [(0u32, 0u8); 256];
    let (mut code, mut previous_length) = (0u32, 0u8);
    for (length, symbol) in pairs {
        if previous_length == 0 {
            previous_length = length;
        }
        code = code
            .checked_shl((length - previous_length) as u32)
            .ok_or(AceError::InvalidHuffman("canonical code overflow"))?;
        if length < 32 && code >= (1u32 << length) {
            return Err(AceError::InvalidHuffman("oversubscribed code lengths"));
        }
        out[symbol as usize] = (code, length);
        code = code.wrapping_add(1);
        previous_length = length;
    }
    Ok(out)
}

/// Decoding tables for one canonical code.
///
/// * `fast`: a `2^LOOKUP_BITS` table indexed by the next `LOOKUP_BITS` stream bits; an entry
///   `(length << 8) | symbol` resolves every code of length `<= LOOKUP_BITS` in one load
///   (`0` = no short code has this prefix).
/// * `first_code` / `count` / `offset` / `symbols`: the classic per-length canonical scheme,
///   used for longer codes and for the stream end, where fewer than `LOOKUP_BITS` bits remain.
pub(crate) struct DecodeTables {
    /// Primary single-lookup table.
    fast: Vec<u16>,
    /// Smallest canonical code of each length (index 0 unused).
    first_code: [u32; MAX_CODE_LEN as usize + 1],
    /// Number of codewords of each length.
    count: [u32; MAX_CODE_LEN as usize + 1],
    /// Index into `symbols` of the first symbol of each length.
    offset: [u32; MAX_CODE_LEN as usize + 1],
    /// Symbols ordered by `(length, symbol)` (= canonical assignment order).
    symbols: Vec<u8>,
}

/// Inherent methods of [`DecodeTables`].
impl DecodeTables {
    /// Builds both tables from a validated length table; fails for an empty alphabet.
    pub(crate) fn new(lengths: &[u8; 256]) -> AceResult<Self> {
        let codes = canonical_codes(lengths)?;
        let mut tables = Self {
            fast: vec![0u16; 1 << LOOKUP_BITS],
            first_code: [0; MAX_CODE_LEN as usize + 1],
            count: [0; MAX_CODE_LEN as usize + 1],
            offset: [0; MAX_CODE_LEN as usize + 1],
            symbols: Vec::new(),
        };
        // One pass over the symbols in canonical (length, symbol) order.
        let mut ordered: Vec<(u8, u8)> = (0..256usize)
            .filter(|&symbol| lengths[symbol] != 0)
            .map(|symbol| (lengths[symbol], symbol as u8))
            .collect();
        ordered.sort_unstable();
        for (length, symbol) in ordered {
            let l = length as usize;
            if tables.count[l] == 0 {
                // The lowest symbol of a length owns that length's smallest code.
                tables.first_code[l] = codes[symbol as usize].0;
                tables.offset[l] = tables.symbols.len() as u32;
            }
            tables.count[l] += 1;
            tables.symbols.push(symbol);
            if length <= LOOKUP_BITS {
                tables.fill_fast(codes[symbol as usize].0, length, symbol);
            }
        }
        if tables.symbols.is_empty() {
            return Err(AceError::InvalidHuffman("empty tree for non-empty output"));
        }
        Ok(tables)
    }

    /// Marks every `LOOKUP_BITS`-bit prefix that starts with `code` as resolving to `symbol`.
    fn fill_fast(&mut self, code: u32, length: u8, symbol: u8) {
        let spare = LOOKUP_BITS - length;
        let start = (code << spare) as usize;
        let entry = ((length as u16) << 8) | symbol as u16;
        self.fast[start..start + (1usize << spare)].fill(entry);
    }

    /// Looks up the next `LOOKUP_BITS` stream bits; returns `(symbol, length)` for short codes.
    #[inline(always)]
    pub(crate) fn lookup_fast(&self, prefix: u32) -> Option<(u8, u8)> {
        let entry = self.fast[prefix as usize];
        (entry != 0).then_some((entry as u8, (entry >> 8) as u8))
    }

    /// Returns the symbol for a complete `length`-bit `code`, or `None` if it is no codeword.
    #[inline]
    pub(crate) fn lookup_canonical(&self, code: u32, length: u8) -> Option<u8> {
        let l = length as usize;
        let relative = code.wrapping_sub(self.first_code[l]);
        (self.count[l] != 0 && code >= self.first_code[l] && relative < self.count[l])
            .then(|| self.symbols[(self.offset[l] + relative) as usize])
    }
}
