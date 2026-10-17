# Migrating from ACE 0.6.0 to 0.6.1

* **API**: the `ace` crate is unchanged; nothing to change in calling code.
* **Output bytes change.** Hashes of compressed data from 0.6.0 will differ for data that now
  uses Patched FOR, ALP, small-delta RunDelta or the DENSE comparison. Decoded bytes are
  identical. Re-baseline any stored hash of compressed output; keep hashes of decoded data.
* **Format 1.5.** Files that use the new modes declare 1.5 and cannot be read by ACE 0.6.0 or
  earlier (the error names ACE 0.6.1). Files that use neither mode are still 1.3 / 1.4 and stay
  readable by 0.6.0. Upgrade readers before writers when both sides must exchange files.
* **Stats.** `Stats` gains the ALP block counter; existing counters keep their meaning.
* **DENSE.** Compression with `Profile::Dense` now takes about 1.7× as long (it also plans the
  BALANCED choice); the output is never larger than BALANCED's.
* **Contributors.** Unit tests moved to `crates/<crate>/tests/`; `#[cfg(test)]` in `src/` is
  rejected by `tools/ace-code_audit0.6.1.py`. Scripts and workflows carry the suffix 0.6.1
  (CI workflow `ace-ci-0.6.1`): update required status-check names in the repository settings.
