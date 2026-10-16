# ace

The public API of the Adaptive Compression Engine (ACE): a lossless, schema-free, adaptive block
compressor with a deterministic cost model, a self-describing container and a block index for
random access. See the repository documentation (`docs/ARCHITECTURE.md`, `docs/FORMAT.md`,
`docs/COMPATIBILITY.md`) and the crate documentation (`cargo doc -p ace --open`).

Every other crate of the workspace is an internal building block and is not published.

```rust
use ace::{Config, Engine, Profile};

let engine = Engine::new(Config::builder().profile(Profile::Balanced).build()?)?;
let packed = engine.compress(&input)?;
let restored = engine.decompress(&packed)?;
let mut reader = engine.open_range_reader(std::io::Cursor::new(&packed))?;
let slice = reader.read_range(4_000..4_100)?;       // decodes only the touched blocks
```

| Item | Purpose |
|---|---|
| `Engine` | compress, decompress, stream, open a `RangeReader`, `explain` (feature `explain`) |
| `Config`, `ConfigBuilder`, `Profile`, `BlockSizePolicy`, `AccessHint` | validated compression settings |
| `DecodeLimits`, `DecodeLimitsBuilder` | bounds enforced on untrusted files before anything is allocated |
| `RangeReader`, `RangeStats` | random access over `Read + Seek` |
| `Stats` | what a run did |
| `Error`, `ErrorKind`, `Result` | failures, classified for the caller |

Migrating from the 0.5 crates: `docs/MIGRATION-0.5-TO-0.6.md`.
