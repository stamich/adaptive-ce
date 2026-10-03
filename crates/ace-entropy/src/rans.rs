use ace_core::{AceError, AceResult};

/// Number of probability bits used by the ACE 0.2/0.2.1 scalar rANS implementation.
pub const RANS_SCALE_BITS: u32 = 12;
/// Sum of every normalized frequency table.
pub const RANS_SCALE: u32 = 1 << RANS_SCALE_BITS;
/// Lower normalization bound for the 32-bit rANS state.
const RANS_L: u32 = 1 << 23;

/// Deterministic normalized frequency model used by the scalar rANS coder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedFrequencyTable {
    /// Per-symbol normalized frequencies that sum exactly to [`RANS_SCALE`].
    pub frequencies: [u16; 256],
    /// Prefix cumulative frequencies with `cumulative[256] == RANS_SCALE`.
    pub cumulative: [u16; 257],
}

impl NormalizedFrequencyTable {
    /// Builds a deterministic normalized model from raw input bytes.
    pub fn from_input(input: &[u8]) -> AceResult<Self> {
        if input.is_empty() { return Ok(Self { frequencies: [0; 256], cumulative: [0; 257] }); }
        let mut counts = [0u64; 256];
        for &b in input { counts[b as usize] += 1; }
        let total = input.len() as u64;
        let mut frequencies = [0u16; 256];
        let mut remainders = Vec::new();
        let mut sum = 0u32;
        for symbol in 0..256usize {
            let count = counts[symbol];
            if count == 0 { continue; }
            let scaled = count * RANS_SCALE as u64;
            let floor = (scaled / total).max(1) as u16;
            frequencies[symbol] = floor;
            sum += floor as u32;
            remainders.push((scaled % total, symbol));
        }
        while sum > RANS_SCALE {
            let mut candidate: Option<usize> = None;
            for symbol in 0..256usize {
                if frequencies[symbol] <= 1 { continue; }
                candidate = match candidate {
                    None => Some(symbol),
                    Some(best) if frequencies[symbol] > frequencies[best] => Some(symbol),
                    Some(best) => Some(best),
                };
            }
            let symbol = candidate.ok_or(AceError::InvalidRans("normalization cannot reduce frequency sum"))?;
            frequencies[symbol] -= 1;
            sum -= 1;
        }
        remainders.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        let mut cursor = 0usize;
        while sum < RANS_SCALE {
            if remainders.is_empty() { return Err(AceError::InvalidRans("empty normalization remainder set")); }
            let symbol = remainders[cursor % remainders.len()].1;
            frequencies[symbol] = frequencies[symbol].saturating_add(1);
            sum += 1;
            cursor += 1;
        }
        Self::from_frequencies(frequencies)
    }

    /// Validates an already normalized frequency array and computes cumulative prefixes.
    pub fn from_frequencies(frequencies: [u16; 256]) -> AceResult<Self> {
        let sum: u32 = frequencies.iter().map(|&v| v as u32).sum();
        if sum != 0 && sum != RANS_SCALE { return Err(AceError::InvalidRans("normalized frequencies do not sum to scale")); }
        let mut cumulative = [0u16; 257];
        let mut running = 0u32;
        for i in 0..256usize {
            cumulative[i] = running as u16;
            running += frequencies[i] as u32;
        }
        cumulative[256] = running as u16;
        Ok(Self { frequencies, cumulative })
    }

    /// Returns the symbol whose cumulative range contains the supplied rANS slot.
    pub fn symbol_for_slot(&self, slot: u32) -> AceResult<u8> {
        if slot >= RANS_SCALE { return Err(AceError::InvalidRans("slot outside normalized scale")); }
        for symbol in 0..256usize {
            let start = self.cumulative[symbol] as u32;
            let end = start + self.frequencies[symbol] as u32;
            if self.frequencies[symbol] != 0 && slot >= start && slot < end { return Ok(symbol as u8); }
        }
        Err(AceError::InvalidRans("slot does not map to a symbol"))
    }

    /// Serializes the model as 256 little-endian `u16` frequencies.
    pub fn encode_metadata(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(512);
        for &frequency in &self.frequencies { out.extend_from_slice(&frequency.to_le_bytes()); }
        out
    }

    /// Parses and validates the fixed 512-byte normalized-frequency metadata.
    pub fn decode_metadata(metadata: &[u8]) -> AceResult<Self> {
        if metadata.len() != 512 { return Err(AceError::InvalidRans("frequency table must contain 512 bytes")); }
        let mut frequencies = [0u16; 256];
        for (i, chunk) in metadata.chunks_exact(2).enumerate() { frequencies[i] = u16::from_le_bytes([chunk[0], chunk[1]]); }
        Self::from_frequencies(frequencies)
    }
}

/// Encodes bytes using a scalar 32-bit range Asymmetric Numeral System.
pub fn rans_encode(input: &[u8]) -> AceResult<(Vec<u8>, Vec<u8>)> {
    if input.is_empty() { return Ok((vec![0u8; 512], Vec::new())); }
    let model = NormalizedFrequencyTable::from_input(input)?;
    let mut state = RANS_L;
    let mut renorm = Vec::new();
    for &symbol in input.iter().rev() {
        let frequency = model.frequencies[symbol as usize] as u32;
        let start = model.cumulative[symbol as usize] as u32;
        if frequency == 0 { return Err(AceError::InvalidRans("input symbol has zero frequency")); }
        let x_max = ((RANS_L >> RANS_SCALE_BITS) << 8).saturating_mul(frequency);
        while state >= x_max {
            renorm.push((state & 0xff) as u8);
            state >>= 8;
        }
        state = ((state / frequency) << RANS_SCALE_BITS) + (state % frequency) + start;
    }
    let mut payload = Vec::with_capacity(4 + renorm.len());
    payload.extend_from_slice(&state.to_le_bytes());
    payload.extend(renorm.iter().rev().copied());
    Ok((model.encode_metadata(), payload))
}

/// Decodes a scalar rANS payload into exactly `expected_size` bytes.
pub fn rans_decode(metadata: &[u8], input: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    if expected_size == 0 { return Ok(Vec::new()); }
    if input.len() < 4 { return Err(AceError::InvalidRans("truncated initial state")); }
    let model = NormalizedFrequencyTable::decode_metadata(metadata)?;
    if model.cumulative[256] as u32 != RANS_SCALE { return Err(AceError::InvalidRans("empty model for non-empty output")); }
    let mut state = u32::from_le_bytes(input[0..4].try_into().map_err(|_| AceError::InvalidRans("truncated state"))?);
    if state < RANS_L { return Err(AceError::InvalidRans("initial state below normalization bound")); }
    let mut cursor = 4usize;
    let mut out = Vec::with_capacity(expected_size.min(1024 * 1024));
    while out.len() < expected_size {
        let slot = state & (RANS_SCALE - 1);
        let symbol = model.symbol_for_slot(slot)?;
        out.push(symbol);
        let frequency = model.frequencies[symbol as usize] as u32;
        let start = model.cumulative[symbol as usize] as u32;
        state = frequency.saturating_mul(state >> RANS_SCALE_BITS).saturating_add(slot - start);
        while state < RANS_L {
            let next = *input.get(cursor).ok_or(AceError::InvalidRans("truncated renormalization bytes"))? as u32;
            cursor += 1;
            state = (state << 8) | next;
        }
    }
    if cursor != input.len() { return Err(AceError::InvalidRans("trailing rANS payload bytes")); }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies exact rANS round-trip for skewed and full-alphabet inputs.
    #[test]
    fn round_trip() {
        let mut data = vec![b'a'; 20_000];
        data.extend((0u8..=255).cycle().take(10_000));
        let (metadata, payload) = rans_encode(&data).unwrap();
        let decoded = rans_decode(&metadata, &payload, data.len()).unwrap();
        assert_eq!(decoded, data);
    }

    /// Verifies that normalization always sums to the fixed scale.
    #[test]
    fn normalization_sum() {
        let data: Vec<u8> = (0u8..=255).cycle().take(100_000).collect();
        let model = NormalizedFrequencyTable::from_input(&data).unwrap();
        assert_eq!(model.frequencies.iter().map(|&x| x as u32).sum::<u32>(), RANS_SCALE);
    }
}
