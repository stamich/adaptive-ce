# ACE 0.3.1 fuzzing

Fuzzing is kept in the standalone `fuzz/` Cargo project and is excluded from the normal workspace.

Targets:
- `container_decode`
- `index_parse`
- `trailer_parse`
- `range_open`

Run:

```bash
cargo install cargo-fuzz
cargo fuzz run container_decode
cargo fuzz run index_parse
cargo fuzz run trailer_parse
cargo fuzz run range_open
```

The hardening invariant is not successful decoding of arbitrary bytes. The invariant is bounded,
panic-free rejection: no runaway allocation, integer-overflow-driven allocation, infinite loop or
process abort for malformed input.
