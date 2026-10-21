//! Runs the fuzz-target bodies on generated inputs, so they are exercised by every test run.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_ffi::fuzzing::{decode, handles};

/// A small deterministic generator (xorshift64*), so failures reproduce.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn bytes(&mut self, length: usize) -> Vec<u8> {
        (0..length).map(|_| (self.next() >> 56) as u8).collect()
    }
}

fn real_file() -> Vec<u8> {
    let data: Vec<u8> = (0_u32..6000).flat_map(|i| (i / 3).to_le_bytes()).collect();
    let config = ace::Config::builder().block_size(4096).build().unwrap();
    ace::Engine::new(config).unwrap().compress(&data).unwrap()
}

#[test]
fn decode_survives_garbage_and_mutated_files() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    decode(&[]);
    for length in [1, 7, 64, 500, 5000] {
        for _ in 0..8 {
            decode(&rng.bytes(length));
        }
    }
    let file = real_file();
    decode(&file);
    for _ in 0..300 {
        let mut mutated = file.clone();
        for _ in 0..=(rng.next() % 4) {
            let at = (rng.next() as usize) % mutated.len();
            mutated[at] ^= 1 << (rng.next() % 8);
        }
        decode(&mutated);
        let cut = (rng.next() as usize) % mutated.len();
        decode(&mutated[..cut]);
    }
}

#[test]
fn handle_sequences_keep_their_invariants() {
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    handles(&[]);
    for _ in 0..400 {
        let length = 1 + (rng.next() % 96) as usize;
        handles(&rng.bytes(length));
    }
    // Sequences that start with a valid engine reach the deeper operations.
    for _ in 0..200 {
        let mut script = vec![0, 1, 1, 1];
        script.extend(rng.bytes(60));
        handles(&script);
    }
}
