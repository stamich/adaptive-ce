# ACE 0.3 fuzzing targets

The release keeps the existing malformed-container targets and adds the following recommended targets:

- `format_1_2_parser`: arbitrary file/block headers including the new rANS4x ID;
- `rans4x_decoder`: arbitrary metadata, lane sizes and payload truncation;
- `planner_sample_offsets`: arbitrary block sizes and sample policies, asserting in-bounds deterministic ranges;
- `stream_decoder`: arbitrary 1.0/1.1/1.2 streams under strict `DecodeLimits`;
- `stream_encoder_roundtrip`: bounded generated input with exact round-trip and deterministic output;
- `indexed_reader`: malformed AIDX/ACET offsets and spans.

Fuzzing must run with resource limits and must treat panics, unbounded allocation attempts and output beyond declared limits as failures.
