//! Deterministic Huffman tree construction (code lengths only).

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use ace_core::{AceError, AceResult};

use super::canonical::MAX_CODE_LEN;

/// One node of the temporary Huffman tree (leaf when `symbol` is set).
#[derive(Clone)]
struct Node {
    /// Left child index.
    left: Option<usize>,
    /// Right child index.
    right: Option<usize>,
    /// Leaf symbol.
    symbol: Option<u8>,
}

/// Builds per-symbol code lengths for `input` (0 = symbol absent).
///
/// Heap entries are ordered by `(frequency, tie, node)` with a deterministic tie counter, so
/// the result never depends on hash or allocation order. A single-symbol input gets length 1.
pub(crate) fn build_code_lengths(input: &[u8]) -> AceResult<[u8; 256]> {
    let mut frequency = [0u64; 256];
    for &byte in input {
        frequency[byte as usize] += 1;
    }
    let mut nodes: Vec<Node> = Vec::new();
    let mut heap: BinaryHeap<Reverse<(u64, u16, usize)>> = BinaryHeap::new();
    for (symbol, &count) in frequency
        .iter()
        .enumerate()
        .filter(|(_, &count)| count != 0)
    {
        let index = nodes.len();
        nodes.push(Node {
            left: None,
            right: None,
            symbol: Some(symbol as u8),
        });
        heap.push(Reverse((count, symbol as u16, index)));
    }
    let mut lengths = [0u8; 256];
    if heap.len() == 1 {
        let Reverse((_, _, index)) = heap.pop().expect("one entry");
        lengths[nodes[index].symbol.expect("leaf") as usize] = 1;
        return Ok(lengths);
    }
    let mut tie = 256u16;
    while heap.len() > 1 {
        let Reverse((weight_a, _, a)) = heap.pop().expect("len > 1");
        let Reverse((weight_b, _, b)) = heap.pop().expect("len > 1");
        let index = nodes.len();
        nodes.push(Node {
            left: Some(a),
            right: Some(b),
            symbol: None,
        });
        heap.push(Reverse((weight_a + weight_b, tie, index)));
        tie = tie.wrapping_add(1);
    }
    if let Some(Reverse((_, _, root))) = heap.pop() {
        assign_depths(&nodes, root, 0, &mut lengths)?;
    }
    Ok(lengths)
}

/// Recursively assigns leaf depths as code lengths, enforcing [`MAX_CODE_LEN`].
fn assign_depths(nodes: &[Node], index: usize, depth: u8, out: &mut [u8; 256]) -> AceResult<()> {
    if depth > MAX_CODE_LEN {
        return Err(AceError::InvalidHuffman(
            "code length exceeds milestone limit",
        ));
    }
    let node = &nodes[index];
    if let Some(symbol) = node.symbol {
        out[symbol as usize] = depth.max(1);
        return Ok(());
    }
    for child in [node.left, node.right].into_iter().flatten() {
        assign_depths(nodes, child, depth + 1, out)?;
    }
    Ok(())
}
