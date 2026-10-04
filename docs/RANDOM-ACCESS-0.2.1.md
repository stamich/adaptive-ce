# Random-access diagnostics 0.2.1

`AceIndexedDecoder::range_metrics` provides pre-decode physical-work estimates for a logical range:

- `logical_bytes_requested`;
- `physical_bytes_read` (sum of serialized indexed block spans);
- `blocks_touched`;
- `blocks_decoded`;
- `overread_ratio = physical_bytes_read / logical_bytes_requested`.

These metrics intentionally expose the storage-engine trade-off between compression block size and random-access amplification without changing Format 1.1.
