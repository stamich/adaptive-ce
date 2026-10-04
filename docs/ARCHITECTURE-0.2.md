# ACE 0.2 architecture

```text
input
  -> FixedBlockChunker
  -> parallel block workers
       -> BlockAnalyzer
       -> CompressionPlanner
       -> deterministic candidate evaluation
       -> transforms
       -> primary codec
       -> Huffman or rANS
       -> CRC32C
  -> ordered deterministic writer
  -> AIDX block index
  -> ACET trailer
```

Every block remains independently decodable. The encoder may use multiple workers, but results are sorted by stable block identifier before serialization. The decoder never runs the analyzer, candidate generator or cost model.

The repository separates three physical stages that were colocated in ACE 0.1:

1. `ace-transforms`: reversible preprocessing such as byte delta.
2. `ace-codecs`: structural byte representation such as RLE or LZ.
3. `ace-entropy`: final probability coding such as Huffman or rANS.

This separation is required for future transform pipelines, SIMD entropy coders and semantic extensions without coupling unrelated algorithms.
