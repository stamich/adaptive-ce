# ACE feature map — 0.2.1

## Inherited from 0.1
- Fixed independent blocks.
- Analyzer + deterministic physical planning.
- RAW/RLE/Delta/LZ and canonical Huffman.
- CRC32C, bounded decoder and CLI.

## Inherited from 0.2
- Scalar rANS.
- Format 1.1 AIDX/ACET index and random access.
- Bounded parallel block execution.
- Dictionary abstractions.
- Unified JSON benchmark families.

## Added in 0.2.1
- Mandatory/Likely/Exploratory candidate tiers.
- Confidence-aware candidate widening.
- Cost Model V2.1 metadata accounting and recalibrated rANS/LZ costs.
- FAST/BALANCED/DENSE profile recalibration.
- Plan-distribution and stage-timing telemetry.
- Per-block planner/oracle diagnostics and recall-by-class.
- `RangeAccessMetrics` and overread measurement.
- 1/2/4/6/8/12-thread benchmark matrix.
- Official ACE 0.2 baseline files and automated regression gates.
