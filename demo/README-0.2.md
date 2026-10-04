# ACE 0.2 demo

Run all milestone demonstrations from the repository root:

```bash
./demo/run-demo-0.2.sh
```

The first demo compresses heterogeneous data and shows the distribution of RAW/RLE/LZ, DELTA, Huffman and rANS decisions. The second opens the serialized format-1.1 index and decodes one block plus one logical byte range. The third compresses identical input with one and eight workers and asserts that the resulting ACE file is bit-for-bit identical.
