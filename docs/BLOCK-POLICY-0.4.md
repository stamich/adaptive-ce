# ACE 0.4 file-level adaptive block policy

ACE 0.3.1 block-size benchmarks showed that large blocks can improve sequential throughput while
small blocks remain preferable for point/range access. 0.4 therefore introduces a deterministic
file-level policy rather than variable block sizes inside one file.

`BlockSizePolicy::Fixed` preserves the configured block size and remains the default.

`BlockSizePolicy::Auto` analyzes at most the first 4 MiB and selects one block size for the complete
file:
- RandomAccess hint: 256 KiB cap;
- high-confidence numeric structure: 1 MiB;
- sequential/FAST generic input: 512 KiB;
- balanced fallback: 256 KiB.

Non-seekable streaming rejects Auto because selecting a block size before writing the fixed header
requires a deterministic pre-sample. Streaming callers should preselect a fixed block size.
