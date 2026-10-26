//! Deterministic dictionary trainer (ACE 0.8.0).
//!
//! The trainer builds a preset dictionary for the LZ codec from sample inputs. A dictionary is
//! history that precedes the compressed block ([`docs/FORMAT-1.6.md`]), so the best content is
//! what many samples have in common: field names, headers, boilerplate, enumerations.
//!
//! # Algorithm
//!
//! A COVER-style greedy selection with integer arithmetic only:
//!
//! 1. Every 8-byte substring (a *k-gram*) of every sample is hashed. The *document frequency* of
//!    a k-gram is the number of samples that contain it at least once. Only k-grams found in two
//!    or more samples can help, so the others are dropped.
//! 2. Candidate segments (windows of [`TrainerConfig::segment_len`] bytes, every half window)
//!    are scored by the sum of the document frequencies of their distinct k-grams.
//! 3. The best segment is taken, and the frequencies of its k-grams drop to zero so that content
//!    that is already covered is not selected again. Rescoring is lazy (a max-heap of upper
//!    bounds), which keeps the cost close to one pass over the input.
//! 4. Segments are taken until the dictionary is full or no candidate has a positive score. The
//!    best segment is placed last, because the LZ codec reaches the end of the dictionary with
//!    the smallest distances.
//!
//! The result depends only on the samples (in order) and the configuration: no randomness, no
//! hash seeds from the environment, no threads, ties broken by sample index and offset. The same
//! samples give the same bytes on every host.
//!
//! [`docs/FORMAT-1.6.md`]: https://github.com/stamich/adaptive-ce/blob/main/docs/FORMAT-1.6.md

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use ace_core::{AceError, AceResult, DictionaryId};

/// Length of the substrings whose frequency is counted.
const GRAM: usize = 8;

/// Largest dictionary the trainer produces: the cap of the container format and of the LZ window.
pub const MAX_TRAINED_DICTIONARY_SIZE: usize = 65_535;

/// Largest value of [`TrainerConfig::max_sample_bytes`], which bounds the memory of the trainer
/// (about 20 bytes per input byte).
pub const MAX_TRAINING_INPUT: usize = 64 * 1024 * 1024;

/// Parameters of [`train_dictionary`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainerConfig {
    /// Size of the dictionary to produce, `1 ..= MAX_TRAINED_DICTIONARY_SIZE`. The result is
    /// shorter when the samples share less content than that.
    pub dictionary_size: usize,
    /// Length of the segments that are copied into the dictionary, `GRAM ..= dictionary_size`.
    /// Short segments follow the shared content closely, long ones keep more context.
    pub segment_len: usize,
    /// Upper bound on the sample bytes that are read, `1 ..= MAX_TRAINING_INPUT`. Samples are
    /// consumed in order; the sample that crosses the bound is cut and the rest is ignored.
    pub max_sample_bytes: usize,
}

/// Default parameters: a 32 KiB dictionary built from 64-byte segments of at most 8 MiB.
impl Default for TrainerConfig {
    /// See the type documentation.
    fn default() -> Self {
        Self {
            dictionary_size: 32 * 1024,
            segment_len: 64,
            max_sample_bytes: 8 * 1024 * 1024,
        }
    }
}

/// Inherent methods of [`TrainerConfig`].
impl TrainerConfig {
    /// Checks that every field is inside its documented range.
    ///
    /// # Errors
    ///
    /// [`AceError::InvalidConfig`] naming the offending field.
    pub fn validate(&self) -> AceResult<()> {
        if self.dictionary_size == 0 || self.dictionary_size > MAX_TRAINED_DICTIONARY_SIZE {
            return Err(AceError::InvalidConfig(
                "dictionary_size must be 1 to 65535 bytes",
            ));
        }
        if self.segment_len < GRAM || self.segment_len > self.dictionary_size {
            return Err(AceError::InvalidConfig(
                "segment_len must be 8 bytes or more and at most dictionary_size",
            ));
        }
        if self.max_sample_bytes == 0 || self.max_sample_bytes > MAX_TRAINING_INPUT {
            return Err(AceError::InvalidConfig(
                "max_sample_bytes must be 1 byte to 64 MiB",
            ));
        }
        Ok(())
    }
}

/// Mixes the 8 bytes at the start of `window` into a 64-bit hash (SplitMix64 finalizer).
fn gram_hash(window: &[u8]) -> u64 {
    let mut word = [0u8; GRAM];
    word.copy_from_slice(window.get(..GRAM).unwrap_or(&[0; GRAM]));
    let mut x = u64::from_le_bytes(word).wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// The hashes of every k-gram of `bytes`, in position order.
fn gram_hashes(bytes: &[u8]) -> Vec<u64> {
    bytes.windows(GRAM).map(gram_hash).collect()
}

/// Document frequencies of the k-grams that occur in at least two samples, sorted by hash.
struct FrequencyTable {
    /// `(hash, document frequency)`, ascending by hash; a frequency of zero means "covered".
    entries: Vec<(u64, u32)>,
}

/// Inherent methods of [`FrequencyTable`].
impl FrequencyTable {
    /// Counts, for every k-gram, the samples that contain it; keeps those found in two or more.
    fn build(samples: &[&[u8]]) -> Self {
        let mut all: Vec<u64> = Vec::new();
        for sample in samples {
            let mut hashes = gram_hashes(sample);
            hashes.sort_unstable();
            hashes.dedup();
            all.extend(hashes);
        }
        all.sort_unstable();
        let mut entries: Vec<(u64, u32)> = Vec::new();
        let mut index = 0;
        while let Some(&hash) = all.get(index) {
            let run = all
                .get(index..)
                .map_or(0, |tail| tail.iter().take_while(|&&h| h == hash).count());
            if run >= 2 {
                entries.push((hash, u32::try_from(run).unwrap_or(u32::MAX)));
            }
            index = index.saturating_add(run.max(1));
        }
        Self { entries }
    }

    /// Position of `hash` in the table.
    fn find(&self, hash: u64) -> Option<usize> {
        self.entries.binary_search_by_key(&hash, |&(h, _)| h).ok()
    }

    /// Current frequency of `hash` (zero when unknown or covered).
    fn frequency(&self, hash: u64) -> u32 {
        self.find(hash)
            .and_then(|i| self.entries.get(i))
            .map_or(0, |&(_, f)| f)
    }

    /// Sets the frequency of `hash` to zero.
    fn cover(&mut self, hash: u64) {
        if let Some(entry) = self.find(hash).and_then(|i| self.entries.get_mut(i)) {
            entry.1 = 0;
        }
    }
}

/// Exact score of a segment: the frequencies of its distinct, not yet covered k-grams.
fn exact_score(table: &FrequencyTable, segment: &[u8]) -> u64 {
    let mut hashes = gram_hashes(segment);
    hashes.sort_unstable();
    hashes.dedup();
    hashes.iter().map(|&h| u64::from(table.frequency(h))).sum()
}

/// Trains a dictionary from `samples`.
///
/// The samples are typical inputs of one data source (records, documents, messages); the more
/// alike they are, and the more of them, the better the dictionary. The result is at most
/// `config.dictionary_size` bytes and is meant for the LZ codec as preset history.
///
/// # Errors
///
/// * [`AceError::InvalidConfig`] when `config` is out of range, when fewer than two samples are
///   long enough to contain a k-gram, or when the samples share no content (a dictionary would
///   not help).
pub fn train_dictionary(samples: &[&[u8]], config: &TrainerConfig) -> AceResult<Vec<u8>> {
    config.validate()?;

    // Apply the input budget in sample order; a sample that crosses it is cut.
    let mut budget = config.max_sample_bytes;
    let mut used: Vec<&[u8]> = Vec::new();
    for sample in samples {
        if budget == 0 {
            break;
        }
        let take = sample.len().min(budget);
        budget = budget.saturating_sub(take);
        if let Some(part) = sample.get(..take) {
            if part.len() >= GRAM {
                used.push(part);
            }
        }
    }
    if used.len() < 2 {
        return Err(AceError::InvalidConfig(
            "training needs at least two samples of 8 bytes or more",
        ));
    }

    let mut table = FrequencyTable::build(&used);
    if table.entries.is_empty() {
        return Err(AceError::InvalidConfig("the samples share no content"));
    }

    // Candidate windows with an upper-bound score (sum without removing repeats) from prefix
    // sums of the per-position frequencies.
    let stride = (config.segment_len / 2).max(1);
    let mut heap: BinaryHeap<(u64, Reverse<usize>, Reverse<usize>, usize)> = BinaryHeap::new();
    for (sample_index, sample) in used.iter().enumerate() {
        let mut prefix: Vec<u64> = Vec::with_capacity(sample.len());
        let mut sum = 0u64;
        prefix.push(0);
        for hash in gram_hashes(sample) {
            sum = sum.saturating_add(u64::from(table.frequency(hash)));
            prefix.push(sum);
        }
        let mut start = 0usize;
        while start < sample.len() {
            let end = start.saturating_add(config.segment_len).min(sample.len());
            let len = end.saturating_sub(start);
            if len >= GRAM {
                let grams_end = end.saturating_sub(GRAM - 1);
                let high = prefix.get(grams_end).copied().unwrap_or(sum);
                let low = prefix.get(start).copied().unwrap_or(0);
                let bound = high.saturating_sub(low);
                if bound > 0 {
                    heap.push((bound, Reverse(sample_index), Reverse(start), len));
                }
            }
            if end == sample.len() {
                break;
            }
            start = start.saturating_add(stride);
        }
    }

    // Lazy greedy selection: a popped candidate is taken when its exact score still beats the
    // best remaining upper bound, otherwise it goes back with the exact score.
    let mut chosen: Vec<&[u8]> = Vec::new();
    let mut total = 0usize;
    while total < config.dictionary_size {
        let Some((_, Reverse(sample_index), Reverse(start), len)) = heap.pop() else {
            break;
        };
        let Some(segment) = used
            .get(sample_index)
            .and_then(|s| s.get(start..start.saturating_add(len)))
        else {
            continue;
        };
        let exact = exact_score(&table, segment);
        if exact == 0 {
            continue;
        }
        if heap.peek().is_some_and(|top| exact < top.0) {
            heap.push((exact, Reverse(sample_index), Reverse(start), len));
            continue;
        }
        for hash in gram_hashes(segment) {
            table.cover(hash);
        }
        let room = config.dictionary_size.saturating_sub(total);
        let part = segment.get(..len.min(room)).unwrap_or(segment);
        total = total.saturating_add(part.len());
        chosen.push(part);
    }
    if chosen.is_empty() {
        return Err(AceError::InvalidConfig("the samples share no content"));
    }

    // The best segment goes last (smallest LZ distances).
    let mut dictionary = Vec::with_capacity(total);
    for part in chosen.iter().rev() {
        dictionary.extend_from_slice(part);
    }
    Ok(dictionary)
}

/// A stable identifier for dictionary `bytes`: FNV-1a (64 bit) of the content.
///
/// Two dictionaries with the same bytes have the same id on every host, so a trained dictionary
/// can be registered and referenced without a separate naming scheme. The value `0` is never
/// returned.
pub fn dictionary_id_of(bytes: &[u8]) -> DictionaryId {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    DictionaryId(hash.max(1))
}
