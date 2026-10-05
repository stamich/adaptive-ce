# ACE 0.3.1 fuzzing

The workspace excludes `fuzz/` from normal Cargo builds. Run the targets with `cargo-fuzz`:

```bash
cargo install cargo-fuzz
cargo fuzz run container_decode
cargo fuzz run index_parse
cargo fuzz run trailer_parse
cargo fuzz run range_open
```

The 0.3.1 invariant is not that arbitrary bytes decode successfully; it is that malformed input is
rejected without panic, runaway allocation, integer overflow, or infinite loops.
