//! Tiny deterministic xorshift generators (no external dependency, identical on every platform).

/// 32-bit xorshift (13, 17, 5) used by the historical `random` and `mixed` corpora.
#[derive(Debug, Clone)]
pub struct XorShift32 {
    /// Current non-zero state.
    state: u32,
}

impl XorShift32 {
    /// Creates a generator; a zero seed is replaced by a fixed non-zero constant.
    pub fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 0x9E37_79B9 } else { seed },
        }
    }

    /// Advances the state and returns it.
    pub fn next_u32(&mut self) -> u32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        self.state
    }
}

/// 64-bit xorshift (13, 7, 17) used by the jittered timestamp corpora.
#[derive(Debug, Clone)]
pub struct XorShift64 {
    /// Current non-zero state.
    state: u64,
}

impl XorShift64 {
    /// Creates a generator; a zero seed is replaced by a fixed non-zero constant.
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    /// Advances the state and returns it.
    pub fn next_u64(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }
}
