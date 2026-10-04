use ace_core::{AceError, AceResult};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

const MAX_CODE_LEN: u8 = 32;

/// Encodes bytes with canonical Huffman coding.
///
/// Metadata format is a fixed 256-byte array of code lengths. This is intentionally simple for
/// ACE 0.2; later milestones may entropy-code the length table.
pub fn huffman_encode(input: &[u8]) -> AceResult<(Vec<u8>, Vec<u8>)> {
    if input.is_empty() {
        return Ok((vec![0u8; 256], Vec::new()));
    }
    let lengths = build_code_lengths(input)?;
    let codes = canonical_codes(&lengths)?;
    let mut writer = BitWriter::default();
    for &b in input {
        let (code, len) = codes[b as usize];
        writer.write(code, len);
    }
    Ok((lengths.to_vec(), writer.finish()))
}

/// Decodes canonical Huffman payload using the serialized 256-byte length table.
pub fn huffman_decode(metadata: &[u8], input: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    if metadata.len() != 256 {
        return Err(AceError::InvalidHuffman(
            "length table must contain 256 bytes",
        ));
    }
    if expected_size == 0 {
        return Ok(Vec::new());
    }
    let mut lengths = [0u8; 256];
    lengths.copy_from_slice(metadata);
    let codes = canonical_codes(&lengths)?;
    let mut table: Vec<(u32, u8, u8)> = Vec::new();
    for symbol in 0..256usize {
        let (code, len) = codes[symbol];
        if len != 0 {
            table.push((code, len, symbol as u8));
        }
    }
    if table.is_empty() {
        return Err(AceError::InvalidHuffman("empty tree for non-empty output"));
    }
    let mut reader = BitReader::new(input);
    let mut out = Vec::with_capacity(expected_size.min(1024 * 1024));
    while out.len() < expected_size {
        let mut code = 0u32;
        let mut matched = None;
        for len in 1..=MAX_CODE_LEN {
            let bit = reader
                .read_bit()
                .ok_or(AceError::InvalidHuffman("truncated bitstream"))?
                as u32;
            code = (code << 1) | bit;
            if let Some(&(_, _, sym)) = table.iter().find(|&&(c, l, _)| c == code && l == len) {
                matched = Some(sym);
                break;
            }
        }
        out.push(matched.ok_or(AceError::InvalidHuffman("no matching canonical code"))?);
    }
    Ok(out)
}

/// Builds per-symbol Huffman code lengths using a deterministic heap ordering.
fn build_code_lengths(input: &[u8]) -> AceResult<[u8; 256]> {
    #[derive(Clone)]
    struct Node {
        left: Option<usize>,
        right: Option<usize>,
        symbol: Option<u8>,
    }
    let mut freq = [0u64; 256];
    for &b in input {
        freq[b as usize] += 1;
    }
    let mut nodes: Vec<Node> = Vec::new();
    let mut heap: BinaryHeap<Reverse<(u64, u16, usize)>> = BinaryHeap::new();
    for s in 0..256usize {
        if freq[s] != 0 {
            let idx = nodes.len();
            nodes.push(Node {
                left: None,
                right: None,
                symbol: Some(s as u8),
            });
            heap.push(Reverse((freq[s], s as u16, idx)));
        }
    }
    if heap.len() == 1 {
        let mut lengths = [0u8; 256];
        let Reverse((_, _, idx)) = heap.pop().unwrap();
        lengths[nodes[idx].symbol.unwrap() as usize] = 1;
        return Ok(lengths);
    }
    let mut tie = 256u16;
    while heap.len() > 1 {
        let Reverse((fa, _, a)) = heap.pop().unwrap();
        let Reverse((fb, _, b)) = heap.pop().unwrap();
        let idx = nodes.len();
        nodes.push(Node {
            left: Some(a),
            right: Some(b),
            symbol: None,
        });
        heap.push(Reverse((fa + fb, tie, idx)));
        tie = tie.wrapping_add(1);
    }
    let root = heap.pop().unwrap().0 .2;
    let mut lengths = [0u8; 256];
    /// Recursively assigns code lengths to leaf symbols while enforcing the milestone depth bound.
    fn walk(nodes: &[Node], idx: usize, depth: u8, out: &mut [u8; 256]) -> AceResult<()> {
        if depth > MAX_CODE_LEN {
            return Err(AceError::InvalidHuffman(
                "code length exceeds milestone limit",
            ));
        }
        let n = &nodes[idx];
        if let Some(s) = n.symbol {
            out[s as usize] = depth.max(1);
            return Ok(());
        }
        if let Some(l) = n.left {
            walk(nodes, l, depth + 1, out)?;
        }
        if let Some(r) = n.right {
            walk(nodes, r, depth + 1, out)?;
        }
        Ok(())
    }
    walk(&nodes, root, 0, &mut lengths)?;
    Ok(lengths)
}

/// Derives canonical `(code,length)` pairs from a per-symbol length table.
fn canonical_codes(lengths: &[u8; 256]) -> AceResult<[(u32, u8); 256]> {
    let mut pairs: Vec<(u8, u16)> = lengths
        .iter()
        .enumerate()
        .filter_map(|(s, &l)| if l == 0 { None } else { Some((l, s as u16)) })
        .collect();
    if pairs.iter().any(|&(l, _)| l > MAX_CODE_LEN) {
        return Err(AceError::InvalidHuffman("code length too large"));
    }
    pairs.sort_unstable();
    let mut out = [(0u32, 0u8); 256];
    let mut code = 0u32;
    let mut prev_len = 0u8;
    for (len, sym) in pairs {
        if prev_len == 0 {
            prev_len = len;
        }
        if len < prev_len {
            return Err(AceError::InvalidHuffman("non-monotonic canonical lengths"));
        }
        code = code
            .checked_shl((len - prev_len) as u32)
            .ok_or(AceError::InvalidHuffman("canonical code overflow"))?;
        if len < 32 && code >= (1u32 << len) {
            return Err(AceError::InvalidHuffman("oversubscribed code lengths"));
        }
        out[sym as usize] = (code, len);
        code = code.wrapping_add(1);
        prev_len = len;
    }
    Ok(out)
}

/// Minimal MSB-first bit writer used by canonical Huffman encoding.
#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    current: u8,
    used: u8,
}
impl BitWriter {
    /// Writes the least significant `len` bits of `code` in canonical MSB-first order.
    fn write(&mut self, code: u32, len: u8) {
        for shift in (0..len).rev() {
            let bit = ((code >> shift) & 1) as u8;
            self.current = (self.current << 1) | bit;
            self.used += 1;
            if self.used == 8 {
                self.bytes.push(self.current);
                self.current = 0;
                self.used = 0;
            }
        }
    }
    /// Finalizes the writer by zero-padding the final byte.
    fn finish(mut self) -> Vec<u8> {
        if self.used != 0 {
            self.current <<= 8 - self.used;
            self.bytes.push(self.current);
        }
        self.bytes
    }
}

/// Minimal MSB-first bit reader used by canonical Huffman decoding.
struct BitReader<'a> {
    input: &'a [u8],
    bit: usize,
}
impl<'a> BitReader<'a> {
    /// Creates a reader over an encoded Huffman payload.
    fn new(input: &'a [u8]) -> Self {
        Self { input, bit: 0 }
    }
    /// Reads one bit or returns `None` when the payload is exhausted.
    fn read_bit(&mut self) -> Option<u8> {
        if self.bit >= self.input.len() * 8 {
            return None;
        }
        let b = self.input[self.bit / 8];
        let bit = (b >> (7 - (self.bit % 8))) & 1;
        self.bit += 1;
        Some(bit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Verifies canonical Huffman round-trip for skewed and full-alphabet data.
    #[test]
    fn roundtrip() {
        let mut d = vec![b'a'; 1000];
        d.extend((0u8..=255).cycle().take(4096));
        let (m, p) = huffman_encode(&d).unwrap();
        assert_eq!(huffman_decode(&m, &p, d.len()).unwrap(), d);
    }
}
