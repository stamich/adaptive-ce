use std::io::{Read, Seek};
use std::ops::Range;
use ace_core::{AceError, AceResult, DecodeLimits};
use ace_index::AceIndexReader;
use crate::block_pipeline::decode_encoded_block;

/// Physical work implied by one logical random-access range request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeAccessMetrics {
    /// Logical bytes requested by the caller.
    pub logical_bytes_requested: u64,
    /// Sum of indexed serialized block spans that must be read.
    pub physical_bytes_read: u64,
    /// Number of independent ACE blocks intersecting the request.
    pub blocks_touched: usize,
    /// Number of independent blocks that must be decoded.
    pub blocks_decoded: usize,
}

/// Inherent methods of [`RangeAccessMetrics`].
impl RangeAccessMetrics {
    /// Returns physical serialized bytes read divided by logical reconstructed bytes requested.
    pub fn physical_to_logical_ratio(&self) -> f64 {
        if self.logical_bytes_requested == 0 { 0.0 } else { self.physical_bytes_read as f64 / self.logical_bytes_requested as f64 }
    }

    /// Backward-compatible alias for the pre-0.3 metric name.
    pub fn overread_ratio(&self) -> f64 { self.physical_to_logical_ratio() }
}

/// High-level random-access decoder backed by the serialized ACE 1.1/1.2 block index.
pub struct AceIndexedDecoder<R: Read + Seek> {
    indexed: AceIndexReader<R>,
    limits: DecodeLimits,
}

/// Inherent methods of [`AceIndexedDecoder`].
impl<R: Read + Seek> AceIndexedDecoder<R> {
    /// Opens and validates an indexed ACE stream.
    pub fn open(reader: R, limits: DecodeLimits) -> AceResult<Self> {
        let indexed = AceIndexReader::open(reader, limits.clone())?;
        Ok(Self { indexed, limits })
    }

    /// Decodes exactly one independent block by stable block identifier.
    pub fn decode_block(&mut self, block_id: u64) -> AceResult<Vec<u8>> {
        let (header, metadata, payload) = self.indexed.read_encoded_block(block_id)?;
        decode_encoded_block(&header, &metadata, &payload, &self.limits)
    }

    /// Returns physical-read diagnostics for a logical range without decoding it.
    pub fn range_metrics(&self, range: Range<u64>) -> AceResult<RangeAccessMetrics> {
        if range.start > range.end || range.end > self.indexed.file_header.original_size {
            return Err(AceError::Malformed("requested range lies outside reconstructed file"));
        }
        let indices = self.indexed.index.intersecting_indices(range.start, range.end);
        let physical_bytes_read = indices.clone().try_fold(0u64, |acc, index| {
            acc.checked_add(self.indexed.index.entries[index].encoded_span as u64)
                .ok_or(AceError::Malformed("physical range byte count overflow"))
        })?;
        let blocks = indices.len();
        Ok(RangeAccessMetrics {
            logical_bytes_requested: range.end.saturating_sub(range.start),
            physical_bytes_read,
            blocks_touched: blocks,
            blocks_decoded: blocks,
        })
    }

    /// Decodes only blocks intersecting the requested logical byte range and trims boundary blocks.
    pub fn read_range(&mut self, range: Range<u64>) -> AceResult<Vec<u8>> {
        self.read_range_with_metrics(range).map(|(bytes, _)| bytes)
    }

    /// Decodes one logical byte range and returns the physical-read diagnostics from the same index lookup.
    ///
    /// ACE 0.3 uses this method to avoid performing one index intersection for
    /// metrics and a second identical lookup for the actual range read. The method does not
    /// reopen or revalidate the container; callers that need open latency should measure
    /// [`AceIndexedDecoder::open`] separately.
    pub fn read_range_with_metrics(
        &mut self,
        range: Range<u64>,
    ) -> AceResult<(Vec<u8>, RangeAccessMetrics)> {
        if range.start > range.end || range.end > self.indexed.file_header.original_size {
            return Err(AceError::Malformed("requested range lies outside reconstructed file"));
        }
        if range.start == range.end {
            return Ok((
                Vec::new(),
                RangeAccessMetrics {
                    logical_bytes_requested: 0,
                    physical_bytes_read: 0,
                    blocks_touched: 0,
                    blocks_decoded: 0,
                },
            ));
        }

        let indices = self.indexed.index.intersecting_indices(range.start, range.end);
        let requested = range.end - range.start;
        if requested > usize::MAX as u64 {
            return Err(AceError::ResourceLimitExceeded("range output size"));
        }

        let physical_bytes_read = indices.clone().try_fold(0u64, |acc, index| {
            acc.checked_add(self.indexed.index.entries[index].encoded_span as u64)
                .ok_or(AceError::Malformed("physical range byte count overflow"))
        })?;
        let blocks = indices.len();
        let metrics = RangeAccessMetrics {
            logical_bytes_requested: requested,
            physical_bytes_read,
            blocks_touched: blocks,
            blocks_decoded: blocks,
        };

        let mut out = Vec::with_capacity(requested as usize);
        for index in indices {
            // Copy only the small fields needed after mutable decoder access; avoid allocating or
            // cloning a temporary vector of complete index entries for every range request.
            let (block_id, block_start, original_size) = {
                let entry = &self.indexed.index.entries[index];
                (entry.block_id, entry.original_offset, entry.original_size)
            };
            let block = self.decode_block(block_id)?;
            let block_end = block_start + original_size as u64;
            let copy_start = range.start.max(block_start) - block_start;
            let copy_end = range.end.min(block_end) - block_start;
            out.extend_from_slice(&block[copy_start as usize..copy_end as usize]);
        }
        Ok((out, metrics))
    }

    /// Returns the number of indexed independent blocks.
    pub fn block_count(&self) -> usize { self.indexed.index.entries.len() }
}
