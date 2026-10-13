# Binary compatibility fixtures

`.ace` files written by earlier ACE releases with **their own encoder**. Every later reader must
decode each of them to its input byte for byte (`crates/ace-engine/tests/compat_fixtures.rs`).

**Rule: these files are never regenerated, edited or deleted.** A new release adds a new
directory; a new format version adds new files. `MANIFEST.json` records for every file the
release, the input case, the writer profile(s) that produced exactly these bytes, the declared
format minor version, the file's SHA-256 and the SHA-256 of the input it must decode to.

The inputs are not stored. The test regenerates them with integer-only generators (six cases:
text, u32 counter, byte runs, f64 series, sparse u64, random) and checks their SHA-256 against
the manifest, so a changed generator cannot silently weaken the fixtures.

| Directory | Written by | Format versions |
|---|---|---|
| `0.2.1/` | 0.2.1-buildfix1 (`8d09846`) | 1.1 |
| `0.3.0/` | 0.3.0 (`c59768b`) | 1.2 |
| `0.4.4/` | 0.4.4 (`0cc0d8e`) | 1.3 (NUM1) |
| `0.4.6/` | 0.4.6 (`e51bd1d`) | 1.3 |
| `0.5.0/` | 0.5.0 (`d07a0a2`, tag `v0.5.0`) | 1.3 and 1.4 (TS1 Gorilla, TS1 RunDelta) |

Format 1.0 is covered by a synthetic file in `crates/ace-engine/tests/format_1_0_compat.rs`
(no released encoder wrote 1.0 with the current block layout).

How the files were produced (once per release, 16 KiB blocks, one thread, profiles FAST /
BALANCED / DENSE; byte-identical profile outputs are stored once):

```bash
ACE_COMPAT_WRITE_INPUTS=/tmp/inputs cargo test -p ace-engine --test compat_fixtures -- --ignored
git worktree add --detach /tmp/ace-0.4.6 e51bd1d
# copy a 30-line example that calls AceEngine::new(AceConfig { block_size: 16384, profile,
# threads: 1, ..AceConfig::default() }).compress(&input) into the old tree, then:
(cd /tmp/ace-0.4.6 && cargo run --release -p ace-engine --example compat_gen -- /tmp/inputs out/0.4.6)
```
