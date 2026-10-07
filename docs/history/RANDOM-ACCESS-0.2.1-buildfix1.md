# Random access hardening in ACE 0.3

The 0.2.1 regression benchmark included repeated decoder open/index-validation cost in the 64-KiB access latency. Buildfix1 separates `decoder_open` from already-open archive access, matching the intended storage-engine usage model.

`AceIndexedDecoder::read_range_with_metrics` returns range bytes and physical-read metrics from the same index intersection. This avoids a duplicate lookup while preserving logical bytes requested, physical bytes read, blocks touched, blocks decoded and overread ratio.
