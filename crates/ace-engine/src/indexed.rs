use crate::block_pipeline::{decode_encoded_block, decode_inner_block};
use crate::dictionaries::DictionaryView;
use ace_core::{AceError, AceResult, CodecId, DecodeLimits, EntropyCodecId};
use ace_dictionary::DictionaryRegistry;
use ace_format::{SegmentTable, INDEX_FLAG_SEGMENTED, SEGMENT_HEADER_SIZE};
use ace_index::AceIndexReader;
use std::io::{Read, Seek};
use std::ops::Range;

/// Physical work implied by one logical random-access range request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeAccessMetrics {
    /// Logical bytes requested by the caller.
    pub logical_bytes_requested: u64,
    /// Bytes read from the file. For unsegmented blocks this is the whole indexed span; for a
    /// segmented block it is the header, the segment table and the segments the range touches,
    /// so it never exceeds the span. [`AceIndexedDecoder::range_metrics`], which reads nothing,
    /// reports the spans (an upper bound).
    pub physical_bytes_read: u64,
    /// Number of independent ACE blocks intersecting the request.
    pub blocks_touched: usize,
    /// Number of independent blocks that must be decoded.
    pub blocks_decoded: usize,
    /// Segments decoded inside segmented blocks (Format 1.6); zero for unsegmented blocks.
    pub segments_decoded: usize,
}

/// Inherent methods of [`RangeAccessMetrics`].
impl RangeAccessMetrics {
    /// Returns physical serialized bytes read divided by logical reconstructed bytes requested.
    pub fn physical_to_logical_ratio(&self) -> f64 {
        if self.logical_bytes_requested == 0 {
            0.0
        } else {
            self.physical_bytes_read as f64 / self.logical_bytes_requested as f64
        }
    }

    /// Backward-compatible alias for the pre-0.3 metric name.
    pub fn overread_ratio(&self) -> f64 {
        self.physical_to_logical_ratio()
    }
}

/// High-level random-access decoder backed by the serialized ACE 1.1/1.2 block index.
pub struct AceIndexedDecoder<R: Read + Seek> {
    indexed: AceIndexReader<R>,
    limits: DecodeLimits,
    /// Dictionaries that blocks of scope `External` are resolved from.
    registry: DictionaryRegistry,
}

/// Inherent methods of [`AceIndexedDecoder`].
impl<R: Read + Seek> AceIndexedDecoder<R> {
    /// Opens and validates an indexed ACE stream.
    ///
    /// # Errors
    ///
    /// Header, trailer and index validation errors ([`AceError::Malformed`],
    /// [`AceError::IndexChecksumMismatch`], [`AceError::ResourceLimitExceeded`]) and
    /// [`AceError::Io`]; files without an index (Format 1.0) are rejected.
    pub fn open(reader: R, limits: DecodeLimits) -> AceResult<Self> {
        let indexed = AceIndexReader::open(reader, limits.clone())?;
        Ok(Self {
            indexed,
            limits,
            registry: DictionaryRegistry::new(),
        })
    }

    /// Sets the registry that blocks of scope `External` are resolved from (Format 1.6).
    ///
    /// Dictionaries embedded in the file (scope `File`) are read by [`Self::open`] and need no
    /// registry; a block that names an external dictionary the registry lacks fails with
    /// [`AceError::MissingDictionary`].
    pub fn with_dictionaries(mut self, registry: DictionaryRegistry) -> Self {
        self.registry = registry;
        self
    }

    /// Decodes exactly one independent block by stable block identifier.
    ///
    /// # Errors
    ///
    /// [`AceError::BlockNotFound`] for an unknown `block_id`; otherwise the block decoding errors of
    /// [`crate::AceEngine::decompress_into`].
    pub fn decode_block(&mut self, block_id: u64) -> AceResult<Vec<u8>> {
        let (header, metadata, payload) = self.indexed.read_encoded_block(block_id)?;
        let dictionaries = DictionaryView::new(&self.indexed.dictionaries, &self.registry, None);
        decode_encoded_block(
            &header,
            &metadata,
            &payload,
            self.indexed.file_header.minor_version,
            &self.limits,
            &dictionaries,
        )
    }

    /// Returns physical-read diagnostics for a logical range without decoding it.
    ///
    /// # Errors
    ///
    /// [`AceError::Malformed`] when the range is reversed or ends past the original size.
    pub fn range_metrics(&self, range: Range<u64>) -> AceResult<RangeAccessMetrics> {
        if range.start > range.end || range.end > self.indexed.file_header.original_size {
            return Err(AceError::Malformed(
                "requested range lies outside reconstructed file",
            ));
        }
        let indices = self
            .indexed
            .index
            .intersecting_indices(range.start, range.end);
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
            segments_decoded: 0,
        })
    }

    /// Decodes only blocks intersecting the requested logical byte range and trims boundary blocks.
    ///
    /// # Errors
    ///
    /// The conditions of [`AceIndexedDecoder::read_range_with_metrics`].
    pub fn read_range(&mut self, range: Range<u64>) -> AceResult<Vec<u8>> {
        self.read_range_with_metrics(range).map(|(bytes, _)| bytes)
    }

    /// Decodes one logical byte range and returns the physical-read diagnostics from the same index lookup.
    ///
    /// ACE 0.3 uses this method to avoid performing one index intersection for
    /// metrics and a second identical lookup for the actual range read. The method does not
    /// reopen or revalidate the container; callers that need open latency should measure
    /// [`AceIndexedDecoder::open`] separately.
    ///
    /// # Errors
    ///
    /// [`AceError::Malformed`] for an invalid range; block decoding errors and
    /// [`AceError::Io`] for every intersecting block.
    pub fn read_range_with_metrics(
        &mut self,
        range: Range<u64>,
    ) -> AceResult<(Vec<u8>, RangeAccessMetrics)> {
        if range.start > range.end || range.end > self.indexed.file_header.original_size {
            return Err(AceError::Malformed(
                "requested range lies outside reconstructed file",
            ));
        }
        if range.start == range.end {
            return Ok((
                Vec::new(),
                RangeAccessMetrics {
                    logical_bytes_requested: 0,
                    physical_bytes_read: 0,
                    blocks_touched: 0,
                    blocks_decoded: 0,
                    segments_decoded: 0,
                },
            ));
        }

        let indices = self
            .indexed
            .index
            .intersecting_indices(range.start, range.end);
        let requested = range.end - range.start;
        if requested > usize::MAX as u64 {
            return Err(AceError::ResourceLimitExceeded("range output size"));
        }

        let blocks = indices.len();
        let mut physical_bytes_read = 0u64;
        let mut segments_decoded = 0usize;
        let mut out = Vec::with_capacity(requested as usize);
        for index in indices {
            // Copy only the small fields needed after mutable decoder access; avoid allocating or
            // cloning a temporary vector of complete index entries for every range request.
            let (block_id, block_start, original_size, span, flags) = {
                let entry = &self.indexed.index.entries[index];
                (
                    entry.block_id,
                    entry.original_offset,
                    entry.original_size,
                    entry.encoded_span,
                    entry.flags,
                )
            };
            let block_end = block_start + original_size as u64;
            let copy_start = (range.start.max(block_start) - block_start) as usize;
            let copy_end = (range.end.min(block_end) - block_start) as usize;
            if flags & INDEX_FLAG_SEGMENTED != 0 {
                let part = self.read_segmented_part(block_id, copy_start..copy_end)?;
                out.extend_from_slice(&part.bytes);
                physical_bytes_read = physical_bytes_read
                    .checked_add(part.physical_bytes)
                    .ok_or(AceError::Malformed("physical range byte count overflow"))?;
                segments_decoded += part.segments;
            } else {
                let block = self.decode_block(block_id)?;
                out.extend_from_slice(&block[copy_start..copy_end]);
                physical_bytes_read = physical_bytes_read
                    .checked_add(u64::from(span))
                    .ok_or(AceError::Malformed("physical range byte count overflow"))?;
            }
        }
        let metrics = RangeAccessMetrics {
            logical_bytes_requested: requested,
            physical_bytes_read,
            blocks_touched: blocks,
            blocks_decoded: blocks,
            segments_decoded,
        };
        Ok((out, metrics))
    }

    /// Reads the part `block_range` of a segmented block (`SEG1`) by fetching its header, its
    /// segment table and only the segments that overlap the range.
    ///
    /// The outer block checksum covers the whole block and is therefore not checked here; every
    /// decoded segment is verified by its own checksum, and the table is validated against the
    /// outer header exactly as in a full read. [`Self::decode_block`] verifies everything.
    fn read_segmented_part(
        &mut self,
        block_id: u64,
        block_range: Range<usize>,
    ) -> AceResult<SegmentedPart> {
        let minor = self.indexed.file_header.minor_version;
        let (header, header_len) = self.indexed.read_block_header_only(block_id)?;
        if header.codec != CodecId::Segmented
            || header.entropy != EntropyCodecId::None
            || !header.transforms.is_empty()
            || header.dictionary.is_some()
            || header.metadata_size != 0
        {
            return Err(AceError::Malformed(
                "block index marks a block as segmented that is not",
            ));
        }
        let payload_offset = header_len as u64;
        let payload_len = header.encoded_size as usize;
        let head = self.indexed.read_within_block(
            block_id,
            payload_offset,
            SEGMENT_HEADER_SIZE.min(payload_len),
        )?;
        let count = SegmentTable::count_from_head(&head, &self.limits)?;
        let table_len = SegmentTable::table_len(count)?;
        if table_len > payload_len {
            return Err(AceError::Malformed("truncated segment table"));
        }
        let prefix = self
            .indexed
            .read_within_block(block_id, payload_offset, table_len)?;
        let table = SegmentTable::parse_prefix(
            &prefix,
            payload_len,
            header.original_size as usize,
            &self.limits,
        )?;
        let mut part = SegmentedPart {
            bytes: Vec::with_capacity(block_range.len()),
            physical_bytes: (header_len + table_len) as u64,
            segments: 0,
        };
        for (index, (stored, logical)) in table
            .payload_ranges()
            .into_iter()
            .zip(table.logical_ranges())
            .enumerate()
        {
            if logical.end <= block_range.start || logical.start >= block_range.end {
                continue;
            }
            let bytes = self.indexed.read_within_block(
                block_id,
                payload_offset + stored.start as u64,
                stored.len(),
            )?;
            let segment_size = u32::try_from(logical.len())
                .map_err(|_| AceError::ResourceLimitExceeded("segment size"))?;
            let dictionaries =
                DictionaryView::new(&self.indexed.dictionaries, &self.registry, None);
            let decoded = decode_inner_block(
                &bytes,
                index as u64,
                segment_size,
                minor,
                &self.limits,
                &dictionaries,
            )?;
            let from = block_range.start.max(logical.start) - logical.start;
            let to = block_range.end.min(logical.end) - logical.start;
            part.bytes.extend_from_slice(&decoded[from..to]);
            part.physical_bytes += stored.len() as u64;
            part.segments += 1;
        }
        Ok(part)
    }

    /// Returns the total reconstructed size of the file in bytes.
    pub fn original_size(&self) -> u64 {
        self.indexed.file_header.original_size
    }

    /// Returns the number of indexed independent blocks.
    pub fn block_count(&self) -> usize {
        self.indexed.index.entries.len()
    }
}

/// The bytes of a range inside one segmented block and what it cost to read them.
struct SegmentedPart {
    /// The requested bytes of the block.
    bytes: Vec<u8>,
    /// Bytes read from the file: header, segment table and the touched segments.
    physical_bytes: u64,
    /// Segments decoded.
    segments: usize,
}
