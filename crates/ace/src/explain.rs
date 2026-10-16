//! Why the planner chose what it chose (feature `explain`).

use ace_engine::BlockExplanation;

use crate::{Engine, Error, Result};

/// The planner's decision for one block.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BlockReport {
    /// Block number, counted from zero.
    pub block_id: u64,
    /// Size of the block in bytes.
    pub size: usize,
    /// The route the block was classified to (for example `generic` or `numeric-fast`).
    pub route: String,
    /// The pipeline that was selected, as a label such as `lz_balanced+huffman`.
    pub selected: String,
    /// Why it was selected.
    pub reason: String,
    /// The label of every pipeline that was considered, best first.
    pub candidates: Vec<String>,
}

impl BlockReport {
    fn from_core(e: &BlockExplanation) -> Self {
        Self {
            block_id: e.block_id,
            size: e.profile.size,
            route: e.route.route.label().to_owned(),
            selected: e.selected.label(),
            reason: e.selected.reason.to_owned(),
            candidates: e.candidates.iter().map(|c| c.label()).collect(),
        }
    }
}

impl Engine {
    /// Reports, per block, which pipeline the planner selects and which it considered. Nothing is
    /// written; every candidate is evaluated, so this is much slower than
    /// [`compress`](Engine::compress). Available with the `explain` feature.
    ///
    /// ```
    /// let engine = ace::Engine::default();
    /// let report = engine.explain(&b"abcabcabc".repeat(5000))?;
    /// assert_eq!(report.len(), 1);
    /// assert!(!report[0].selected.is_empty());
    /// # Ok::<(), ace::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`ErrorKind::InvalidConfig`](crate::ErrorKind::InvalidConfig) for unusable settings.
    pub fn explain(&self, input: &[u8]) -> Result<Vec<BlockReport>> {
        let explanations = self.internal().explain(input).map_err(Error::from_core)?;
        Ok(explanations.iter().map(BlockReport::from_core).collect())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::Config;

    #[test]
    fn the_report_names_the_pipeline_that_compress_uses() {
        let data: Vec<u8> = (0..100_000u32)
            .flat_map(|i| (i * 7).to_le_bytes())
            .collect();
        let engine =
            Engine::new(Config::builder().block_size(128 * 1024).build().unwrap()).unwrap();
        let report = engine.explain(&data).unwrap();
        let (_, stats) = engine.compress_with_stats(&data).unwrap();
        assert_eq!(report.len() as u64, stats.blocks);
        for r in &report {
            assert!(r.candidates.contains(&r.selected), "{r:?}");
            assert!(stats.plan_distribution.contains_key(&r.selected), "{r:?}");
        }
        assert_eq!(report[0].block_id, 0);
    }
}
