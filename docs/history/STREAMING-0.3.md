# Bounded streaming in ACE 0.3

`ace-stream` provides `compress_reader_known_size` and `decompress_stream`.

ACE's fixed header contains original byte size and block count, therefore a non-seekable writer needs the expected input size before compression starts. The encoder verifies that the reader supplies exactly that size.

Only one source block is retained at a time. The normal block format is emitted in order; the block index is accumulated as compact entries and serialized before the existing trailer. Completed output therefore remains compatible with `AceIndexedDecoder`.

The 0.3 implementation intentionally prioritizes bounded memory and semantic reuse of the regular engine over maximum streaming throughput. A later runtime milestone can replace the temporary one-block internal container with a dedicated public block encoder API without changing the wire format.
