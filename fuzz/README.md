# ACE fuzzing

The `fuzz/` project is excluded from normal workspace builds and is intended for `cargo-fuzz`.

ACE 0.3.1 targets:
- `container_decode`
- `index_parse`
- `trailer_parse`
- `range_open`

ACE 0.4 adds:
- `numeric_decode`
- `bitpack_decode`

Run:

```bash
cargo install cargo-fuzz
cargo fuzz run container_decode
cargo fuzz run index_parse
cargo fuzz run trailer_parse
cargo fuzz run range_open
cargo fuzz run numeric_decode
cargo fuzz run bitpack_decode
```

Malformed inputs are expected to return errors. The hard invariant is bounded, panic-free rejection:
no runaway allocation, integer-overflow-driven allocation, infinite loop or process abort.
