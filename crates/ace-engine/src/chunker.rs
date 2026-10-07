/// Deterministic fixed-size block splitter retained by ACE 0.2.1.
#[derive(Debug, Clone, Copy)]
pub struct FixedBlockChunker {
    block_size: usize,
}

/// Inherent methods of [`FixedBlockChunker`].
impl FixedBlockChunker {
    /// Creates a chunker or returns `None` for a zero block size.
    pub fn new(block_size: usize) -> Option<Self> {
        (block_size != 0).then_some(Self { block_size })
    }
    /// Returns an iterator over independent fixed-size source blocks.
    pub fn chunks<'a>(&self, input: &'a [u8]) -> impl Iterator<Item = &'a [u8]> {
        input.chunks(self.block_size)
    }
}
