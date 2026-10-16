# ace

The public API of the Adaptive Compression Engine (ACE): a lossless, schema-free, adaptive block
compressor with a deterministic cost model, a self-describing container and a block index for
random access. See the repository documentation (`docs/ARCHITECTURE.md`, `docs/FORMAT.md`,
`docs/COMPATIBILITY.md`) and the crate documentation (`cargo doc -p ace --open`).

Every other crate of the workspace is an internal building block and is not published.
