# ACE 0.1 architecture

ACE 0.1 implements a single-threaded adaptive block pipeline:

`FixedBlockChunker -> BlockAnalyzer -> CompressionPlanner -> CandidateEvaluator -> Executor -> SizeGuard -> ACE Format`

Blocks are independent. The decoder consumes only the serialized decoding plan and never runs analysis or planning.
DELTA is represented as a transform rather than a codec so later transform pipelines can evolve without changing the
conceptual execution model.

The format-level checksum is CRC32C over reconstructed original bytes. It detects storage/transport corruption and
codec defects; it is not a cryptographic integrity mechanism.
