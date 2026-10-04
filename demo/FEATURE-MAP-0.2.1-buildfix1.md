# Feature map — ACE 0.2.1-buildfix1

## Inherited from 0.2/0.2.1
- scalar rANS and canonical Huffman;
- independent blocks and deterministic parallel execution;
- AIDX/ACET block index and random access;
- Format 1.0/1.1 read compatibility and 1.1 writer;
- candidate tiers, plan distribution and regression benchmark contract.

## Buildfix1
- profile-separated candidate generation;
- FAST without balanced LZ or mandatory rANS;
- BALANCED/DENSE full oracle-family coverage;
- normalized Cost Model V2.1 score;
- `read_range_with_metrics()` single-lookup path;
- explicit `decoder_open` benchmark;
- regression comparison against both 0.2 and observed 0.2.1.
