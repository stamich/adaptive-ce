#!/usr/bin/env python3
"""ACE 0.8.0 code audit: unsafe placement, panic policy and decoder hardening.

Checks (exit 1 on violation):

* ``unsafe`` appears only in the audited files (``UNSAFE_ALLOWED``), and every ``unsafe``
  block / impl is preceded by a ``// SAFETY:`` comment within six lines;
* every ``#[allow(...clippy::...)]`` in production code (``crates/*/src`` outside
  ``#[cfg(test)]`` modules) carries a ``//`` justification on the same line, in the comment
  block directly above it, or on the line directly below it (doc comments do not count);
* no ``#[cfg(test)]`` appears in ``crates/*/src``: tests live in ``crates/<crate>/tests/<module>.rs``
  and use public items only (``docs/ARCHITECTURE.md``, "Test layout");
* every ``pub`` item of the public API crate (``PUBLIC_CRATES``: ``ace``) has a doc comment, and
  no ``#[deprecated]`` item remains anywhere in the workspace (deprecated items are removed
  with the 0.6 facade);
* every parser module listed in ``DECODER_MODULES`` enables the decoder-hardening lints, and
  every test named in the decoder field table (``DECODER_FIELDS``) and every fuzz target named
  for the unlinted hot loops (``HOT_LOOPS``) exists.

* every ``#[no_mangle]`` function of ``crates/ace-ffi`` runs under ``guard`` / ``guard_value``
  (panic containment) unless it is one of the ``FFI_INFALLIBLE`` accessors, is declared in
  ``include/ace.h``, is called by a test in ``crates/ace-ffi/tests``, and the fuzz targets
  ``ffi_decode`` and ``ffi_handles`` exist.

Writes the generated ``docs/PANIC-AUDIT.md``, ``docs/DECODER-AUDIT.md`` and
``docs/API-INVENTORY.md`` (``--write``).

usage: ace-code_audit0.8.0.py [--write]
"""
from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
UNSAFE_ALLOWED = {
    "crates/ace-simd/src/avx2.rs",
    "crates/ace-simd/src/crc32c.rs",
    "crates/ace-simd/src/scan.rs",
    "crates/ace-bench/src/alloc.rs",
    "crates/ace-bench/src/families/ffi.rs",
    "crates/ace-bench/tests/limits_before_allocation.rs",
}
#: Directories whose every file may contain ``unsafe`` (still needing ``// SAFETY:``): the C ABI
#: crate dereferences host pointers in each of its modules and tests.
UNSAFE_ALLOWED_DIRS = ("crates/ace-ffi/src/", "crates/ace-ffi/tests/")
UNSAFE_USE = re.compile(r"\bunsafe\s*(\{|impl\b|fn\b)")
ALLOW = re.compile(r"#\[allow\(((?:[a-z_]+,\s*)*clippy::[a-z_]+(?:,\s*(?:clippy::)?[a-z_]+)*)\)\]")
HARDENING_LINT = "#![warn(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]"
PANIC_LINTS = ("unwrap_used", "expect_used", "panic", "todo", "unimplemented", "unreachable")


#: Parser modules that must enable the decoder-hardening lints (``HARDENING_LINT``).
DECODER_MODULES = (
    "crates/ace-format/src/header.rs",
    "crates/ace-format/src/index.rs",
    "crates/ace-format/src/block_io.rs",
    "crates/ace-index/src/index_reader.rs",
    "crates/ace-engine/src/block_pipeline.rs",
    "crates/ace-codecs/src/numeric/header.rs",
    "crates/ace-codecs/src/numeric/decode.rs",
    "crates/ace-codecs/src/numeric/patched_decode.rs",
    "crates/ace-codecs/src/time_series/header.rs",
    "crates/ace-entropy/src/rans/model.rs",
    "crates/ace-entropy/src/rans4x.rs",
    "crates/ace-bitpack/src/scalar.rs",
)

#: Hot decode loops deliberately outside the arithmetic lint: every index there is bounded by a
#: structure validated up front, and the module is covered by the named fuzz targets.
HOT_LOOPS = (
    ("crates/ace-bitpack/src/bitstream.rs",
     "`BitReader::new` requires `input.len() == ceil(bit_len / 8)`; every read is bounds-checked",
     ("bitstream_roundtrip", "ts1_decode")),
    ("crates/ace-codecs/src/lz.rs",
     "token lengths use `checked_add` / `get`; output never exceeds `expected_size`",
     ("container_decode", "engine_roundtrip")),
    ("crates/ace-codecs/src/rle.rs",
     "run / literal lengths use `checked_add` / `get`; output never exceeds `expected_size`",
     ("rle_roundtrip", "container_decode")),
    ("crates/ace-codecs/src/time_series/gorilla.rs",
     "all bits come from a validated `BitReader`; every window is checked "
     "(`leading + meaningful <= lane bits`); no bits may remain",
     ("ts1_decode", "ts1_roundtrip")),
    ("crates/ace-codecs/src/time_series/run_delta.rs",
     "gamma run lengths are checked against the remaining value count; deltas must fit the "
     "lane; no bits may remain",
     ("ts1_decode", "ts1_roundtrip")),
    ("crates/ace-entropy/src/rans/coder.rs",
     "renormalization reads use `get`; final state and trailing bytes are checked",
     ("rans_decoder", "rans4x_decoder")),
    ("crates/ace-entropy/src/huffman/decoder.rs",
     "canonical tables validated by `DecodeTables::new`; reads past the end are zero bits "
     "compared against `remaining()`",
     ("container_decode",)),
)

#: Decoder field table: (structure, field, validation, limit, tests). A test is
#: ``path::function``; the audit fails when it does not exist.
DECODER_FIELDS = (
    ("File header", "magic, major / minor version",
     "exact magic; major 1, minor <= 4", "-",
     ("crates/ace-engine/tests/malformed_matrix.rs::mutation_table_is_rejected",
      "crates/ace-format/tests/format_1_4.rs::file_headers_1_0_to_1_5_are_readable")),
    ("File header", "flags (u16)", "only known bits (1.1+), else `UnsupportedFeature`", "-",
     ("crates/ace-engine/tests/malformed_matrix.rs::mutation_table_is_rejected",)),
    ("File header", "default_block_size (u32)", "informational; never sizes an allocation", "-",
     ("crates/ace-engine/tests/malformed_matrix.rs::informational_block_size_field_does_not_affect_decoding",)),
    ("File header", "original_size (u64)",
     "<= limit; reservation capped by `PREALLOCATION_CAP_BYTES`; must equal the sum of the blocks",
     "`max_output_size`",
     ("crates/ace-engine/tests/malformed_matrix.rs::forged_original_size_is_not_preallocated",
      "crates/ace-bench/tests/limits_before_allocation.rs::forged_original_size_fails_before_allocation")),
    ("File header", "block_count (u64)",
     "<= limit before the first block; ids sequential; equals the index entry count",
     "`max_index_entries` (`max_block_count()`)",
     ("crates/ace-engine/tests/decode_limits.rs::block_count_limit",
      "crates/ace-engine/tests/malformed_matrix.rs::mutation_table_is_rejected")),
    ("Block header", "original_size (u32)",
     "<= limit before decoding; every decoder must produce exactly this many bytes",
     "`max_block_size`",
     ("crates/ace-engine/tests/decode_limits.rs::size_limits",
      "crates/ace-bench/tests/limits_before_allocation.rs::forged_block_sizes_fail_before_allocation")),
    ("Block header", "encoded_size, metadata_size (u32)",
     "`checked_add`, then <= limit before either buffer is allocated", "`max_encoded_block_size`",
     ("crates/ace-engine/tests/decode_limits.rs::size_limits",
      "crates/ace-bench/tests/limits_before_allocation.rs::forged_block_sizes_fail_before_allocation")),
    ("Block header", "transform count (u8)",
     "<= limit before the descriptors are allocated; descriptors split with `split_at_checked`",
     "`max_transforms`",
     ("crates/ace-engine/tests/malformed_matrix.rs::resource_limits_are_enforced",
      "crates/ace-format/tests/index_hardening.rs::writer_rejects_more_than_255_transforms")),
    ("Block header", "codec / entropy / transform ids",
     "`TryFrom` (unknown = error); rANS4x >= 1.2, NUM1 >= 1.3, TS1 >= 1.4 and plain", "-",
     ("crates/ace-format/tests/format_1_3.rs::numeric_codec_requires_format_1_3",
      "crates/ace-format/tests/format_1_4.rs::time_series_codec_requires_format_1_4",
      "crates/ace-format/tests/format_1_4.rs::time_series_block_must_be_plain")),
    ("Block header", "header CRC, payload CRC-32C",
     "header CRC before any field is used; payload CRC over the decoded block", "-",
     ("crates/ace-engine/tests/malformed_matrix.rs::single_byte_flip_sweep_never_misdecodes",)),
    ("Entropy metadata", "primary (codec) stream length (u32)",
     "`try_from`; <= 2 x block original size + 1 KiB before the entropy decoder runs (a "
     "probability-one rANS model decodes any length from four bytes)", "`max_block_size` (derived)",
     ("crates/ace-bench/tests/limits_before_allocation.rs::forged_primary_stream_length_fails_before_allocation",)),
    ("Index (AIDX)", "entry count (u64)",
     "`try_from` to `usize`, <= limit, then `checked_mul` — all before allocation",
     "`max_index_entries`",
     ("crates/ace-format/tests/index_hardening.rs::extreme_entry_counts_are_rejected_before_allocation",
      "crates/ace-format/tests/index_hardening.rs::index_entry_limit_is_enforced")),
    ("Index (AIDX)", "entries",
     "fixed 40-byte records via `as_chunks`; ids strictly increasing; offsets contiguous "
     "(`checked_add`); spans inside the block region", "-",
     ("crates/ace-format/tests/index_hardening.rs::non_contiguous_logical_offsets_are_rejected",
      "crates/ace-format/tests/index_hardening.rs::duplicate_block_ids_are_rejected")),
    ("Trailer (ACET)", "index_offset, index_size (u64)",
     "CRC; `checked_add`; inside the file; size `try_from` and <= 12 + 40 x limit before "
     "allocation", "`max_index_entries`",
     ("crates/ace-index/tests/trailer_limits.rs::index_size_above_the_entry_limit_is_refused",
      "crates/ace-index/tests/trailer_limits.rs::forged_index_sizes_are_rejected",
      "crates/ace-format/tests/index_hardening.rs::trailer_crc_corruption_is_rejected")),
    ("NUM1 header", "width, mode, bit width, first value / delta",
     "known values; bit width <= lane bits; first value and `|first delta|` fit the lane", "-",
     ("crates/ace-codecs/tests/header_fields.rs::num1_first_delta_and_bit_width_extremes",)),
    ("NUM1 header", "value_count, tail_bytes, packed_bytes (u32)",
     "`try_from`; header + packed + tail (`checked_add`) == payload; value_count x width + tail "
     "(`checked_mul` / `checked_add`) == expected size before allocation",
     "`max_block_size` (expected size)",
     ("crates/ace-codecs/tests/header_fields.rs::num1_count_fields_reject_zero_max_and_overflow",
      "crates/ace-codecs/tests/header_fields.rs::num1_and_ts1_reject_expected_size_mismatch")),
    ("NUM1 header (mode 4)", "patch source, exception count (u32), exception width byte",
     "source is FOR / Delta / DoD; count <= source stream length; width in 1..=lane bits - "
     "bit width; section length (`checked_add`) known before the total length check", "-",
     ("crates/ace-codecs/tests/num1_patched.rs::malformed_patched_payloads_are_rejected",)),
    ("NUM1 exception section", "positions (bit-packed), high parts (bit-packed)",
     "positions strictly increasing and inside the stream; high parts non-zero; both streams "
     "validated before the first value is produced", "-",
     ("crates/ace-codecs/tests/num1_patched.rs::malformed_patched_payloads_are_rejected",
      "crates/ace-codecs/tests/num1_patched.rs::damaged_payloads_never_panic")),
    ("TS1 header", "magic, version, mode, lane width, flags, reserved",
     "exact; lane width valid for the mode; flags allowed per mode; reserved bytes zero", "-",
     ("crates/ace-codecs/tests/header_fields.rs::ts1_small_fields_are_validated",)),
    ("TS1 header", "value_count (u32), stream_bits (u32), tail_len (u8)",
     "`try_from`; tail < lane; header + ceil(bits / 8) + tail (`checked_add`) == payload; "
     "decoded length (`checked_*`) == expected size before allocation",
     "`max_block_size` (expected size)",
     ("crates/ace-codecs/tests/header_fields.rs::ts1_count_fields_reject_zero_max_and_overflow",
      "crates/ace-codecs/tests/header_fields.rs::num1_and_ts1_reject_expected_size_mismatch")),
    ("rANS metadata", "256 x u16 frequencies",
     "exactly 512 bytes; sum (u32) == 4096 before prefixes; prefixes `checked_add` in u16", "-",
     ("crates/ace-entropy/tests/metadata_fields.rs::rans_frequency_tables_with_bad_sums_are_rejected",)),
    ("rANS stream", "state + renormalization bytes",
     "initial state >= L; reads via `get`; no trailing bytes; final state == L; empty output "
     "needs an empty payload", "expected size",
     ("crates/ace-entropy/tests/metadata_fields.rs::scalar_coders_reject_extreme_output_sizes",)),
    ("rANS4x metadata", "4 x u32 lane lengths",
     "`try_from`; `checked_add` sum == payload; lanes split with `split_at_checked`; lane output "
     "sizes computed without overflow", "expected size",
     ("crates/ace-entropy/tests/metadata_fields.rs::rans4x_lane_lengths_reject_zero_max_and_overflow",)),
    ("Huffman metadata", "256 code lengths + stream",
     "exactly 256 bytes; valid canonical code; truncation is an error; only zero padding (< 8 "
     "bits) may remain; empty output needs an empty payload", "expected size",
     ("crates/ace-entropy/tests/metadata_fields.rs::scalar_coders_reject_extreme_output_sizes",)),
)


def production_lines(path: pathlib.Path) -> list[tuple[int, str]]:
    """Numbered lines of ``path`` before its first ``#[cfg(test)]`` (tests are exempt)."""
    lines = []
    for number, line in enumerate(path.read_text().splitlines(), start=1):
        if line.strip().startswith("#[cfg(test)]"):
            break
        lines.append((number, line))
    return lines


def rust_sources() -> list[pathlib.Path]:
    """Every production Rust source of the workspace."""
    return sorted(ROOT.glob("crates/*/src/**/*.rs")) + sorted(ROOT.glob("crates/*/build.rs"))


def unsafe_sources() -> list[pathlib.Path]:
    """Every Rust source that may contain ``unsafe``: production code, tests and benches."""
    return rust_sources() + sorted(ROOT.glob("crates/*/tests/**/*.rs"))


def audit_unsafe(problems: list[str]) -> list[tuple[str, int, str]]:
    """Unsafe sites as ``(file, line, kind)``; appends policy violations to ``problems``."""
    sites = []
    for path in unsafe_sources():
        relative = path.relative_to(ROOT).as_posix()
        lines = path.read_text().splitlines()
        for index, line in enumerate(lines):
            code = line.split("//", 1)[0]
            match = UNSAFE_USE.search(code)
            if not match:
                continue
            kind = {"{": "block", "impl": "impl", "fn": "fn"}[match.group(1).split()[0]]
            sites.append((relative, index + 1, kind))
            if relative not in UNSAFE_ALLOWED and not relative.startswith(UNSAFE_ALLOWED_DIRS):
                problems.append(f"{relative}:{index + 1}: unsafe outside the audited files")
            if kind in ("block", "impl"):
                context = "\n".join(lines[max(0, index - 6):index + 1])
                if "SAFETY:" not in context:
                    problems.append(f"{relative}:{index + 1}: unsafe {kind} without // SAFETY: comment")
    return sites


def is_plain_comment(line: str) -> bool:
    """A ``//`` comment line that is not a doc comment."""
    text = line.strip()
    return text.startswith("//") and not text.startswith(("///", "//!"))


def justification_of(numbered: list[tuple[int, str]], position: int, line: str) -> str:
    """The ``//`` text justifying the attribute on ``line`` (same line, block above, line below)."""
    same = line.split(")]", 1)[1] if ")]" in line else ""
    if "//" in same:
        return same.split("//", 1)[1].strip()
    above = []
    cursor = position - 1
    while cursor >= 0 and is_plain_comment(numbered[cursor][1]):
        above.insert(0, numbered[cursor][1].strip().lstrip("/").strip())
        cursor -= 1
    if above:
        return " ".join(above)
    below = []
    cursor = position + 1
    while cursor < len(numbered) and is_plain_comment(numbered[cursor][1]):
        below.append(numbered[cursor][1].strip().lstrip("/").strip())
        cursor += 1
    return " ".join(below)


def audit_allows(problems: list[str]) -> list[tuple[str, int, str, str]]:
    """Production ``allow(clippy::..)`` sites as ``(file, line, lints, justification)``."""
    sites = []
    for path in rust_sources():
        relative = path.relative_to(ROOT).as_posix()
        numbered = production_lines(path)
        for position, (number, line) in enumerate(numbered):
            match = ALLOW.search(line)
            if not match:
                continue
            justification = justification_of(numbered, position, line)
            if not justification.strip():
                problems.append(f"{relative}:{number}: #[allow({match.group(1)})] without justification")
            sites.append((relative, number, match.group(1), justification.strip()))
    return sites


def audit_test_layout(problems: list[str]) -> None:
    """Production sources contain no ``#[cfg(test)]`` item; tests live in ``crates/*/tests``."""
    for path in rust_sources():
        for number, line in enumerate(path.read_text().splitlines(), start=1):
            if re.match(r"\s*#\[cfg\((?:.*\b)?test\b", line):
                problems.append(
                    f"{path.relative_to(ROOT)}:{number}: #[cfg(test)] in src; "
                    "move the tests to crates/<crate>/tests/<module>.rs"
                )


def test_exists(reference: str) -> bool:
    """Whether ``path::function`` names an existing test function."""
    path, _, function = reference.partition("::")
    file = ROOT / path
    return file.is_file() and re.search(rf"\bfn {re.escape(function)}\(", file.read_text()) is not None


def audit_decoder(problems: list[str]) -> None:
    """Hardening lints present, referenced tests and fuzz targets exist."""
    for module in DECODER_MODULES:
        if HARDENING_LINT not in (ROOT / module).read_text():
            problems.append(f"{module}: decoder module without the hardening lints")
    for structure, field, _, _, tests in DECODER_FIELDS:
        for test in tests:
            if not test_exists(test):
                problems.append(f"decoder audit: {structure} / {field}: missing test {test}")
    fuzz = (ROOT / "fuzz" / "Cargo.toml").read_text()
    for module, _, targets in HOT_LOOPS:
        for target in targets:
            if f'name = "{target}"' not in fuzz:
                problems.append(f"decoder audit: {module}: missing fuzz target {target}")


def render_decoder(allows: list[tuple[str, int, str, str]]) -> str:
    """Markdown of the generated decoder audit."""
    lines = [
        "# ACE decoder audit",
        "",
        "> Generated by `tools/ace-code_audit0.8.0.py --write`; do not edit by hand. The audit",
        "> fails when a listed test or fuzz target disappears or a parser module loses its lints.",
        "",
        "## Rule",
        "",
        "Every integer read from a file reaches an allocation only through",
        "`external integer → try_from → checked_mul / checked_add → DecodeLimits → allocation`.",
        "Sizes that depend on the caller's expected output (block original size) are compared",
        "with it before the output buffer exists. A forged value is an error, never a panic, a",
        "wrap-around or an allocation of the forged size",
        "(`crates/ace-bench/tests/limits_before_allocation.rs` measures the last point).",
        "",
        "## Fields",
        "",
        "| Structure | Field | Validation | Limit | Tests |",
        "|---|---|---|---|---|",
    ]
    for structure, field, validation, limit, tests in DECODER_FIELDS:
        names = "<br>".join(f"`{t.split('/tests/', 1)[-1]}`" for t in tests)
        lines.append(f"| {structure} | {field} | {validation} | {limit} | {names} |")
    lines += [
        "",
        "## Parser modules under the arithmetic lints",
        "",
        f"Each module starts with `{HARDENING_LINT}`; `-D warnings` turns any unchecked",
        "arithmetic or narrowing cast into a build failure. Proven-safe sites:",
        "",
        "| Module | Proven-safe allowances |",
        "|---|---|",
    ]
    for module in DECODER_MODULES:
        local = [f"`:{n}` {j}" for f, n, l, j in allows
                 if f == module and ("arithmetic_side_effects" in l or "cast_possible" in l)]
        lines.append(f"| `{module}` | {'<br>'.join(local) if local else 'none'} |")
    lines += [
        "",
        "## Hot decode loops outside the lint",
        "",
        "| Module | Why indices cannot escape | Fuzz targets |",
        "|---|---|---|",
    ]
    for module, reason, targets in HOT_LOOPS:
        lines.append(f"| `{module}` | {reason} | {', '.join(f'`{t}`' for t in targets)} |")
    return "\n".join(lines) + "\n"


#: Crates whose items form the public API. Every other library crate is an internal building
#: block (``publish = false``) whose ``pub`` items exist for the other ACE crates, the benchmark
#: harness and the fuzz targets.
PUBLIC_CRATES = ("ace",)
#: The facade item and the internal item it is built on.
FACADE_06 = (
    ("ace::Engine", "ace_engine::AceEngine (+ ace_stream::compress_reader_known_size)"),
    ("ace::Config, ConfigBuilder, Profile, BlockSizePolicy, AccessHint", "ace_core::AceConfig, CompressionProfile, BlockSizePolicy, AccessHint"),
    ("ace::DecodeLimits, DecodeLimitsBuilder", "ace_core::DecodeLimits"),
    ("ace::RangeReader, RangeStats", "ace_engine::AceIndexedDecoder, RangeAccessMetrics"),
    ("ace::Stats", "ace_core::CompressionStats"),
    ("ace::Error, ErrorKind, Result", "ace_core::AceError, AceResult"),
    ("ace::BlockReport (feature `explain`)", "ace_engine::AceEngine::explain, BlockExplanation"),
)
PUB_ITEM = re.compile(r"^(\s*)pub\s+(?:const\s+fn|async\s+fn|unsafe\s+fn|fn|struct|enum|trait|const|static|type)\s+([A-Za-z_][A-Za-z0-9_]*)")


def public_items(crate: pathlib.Path) -> list[tuple[str, str, int, str, bool, bool]]:
    """``(file, name, line, first doc line, is_method, deprecated)`` of every ``pub`` item."""
    items = []
    for path in sorted((crate / "src").rglob("*.rs")):
        lines = production_lines(path)
        for position, (number, line) in enumerate(lines):
            match = PUB_ITEM.match(line)
            if not match:
                continue
            # The doc comment and attributes directly above the item: everything up to the
            # previous blank line or the end of the previous item.
            docs, deprecated, cursor = [], False, position - 1
            while cursor >= 0:
                text = lines[cursor][1].strip()
                if not text or text.endswith(("}", ";", "{")) and not text.startswith("//"):
                    break
                if text.startswith("///"):
                    docs.insert(0, text[3:].strip())
                deprecated |= text.startswith("#[deprecated")
                cursor -= 1
            first = next((d for d in docs if d), "")
            items.append((path.relative_to(ROOT).as_posix(), match.group(2), number, first,
                          bool(match.group(1)), deprecated))
    return items


def render_api() -> str:
    """Markdown of the generated public-API inventory."""
    lines = [
        "# ACE public API inventory",
        "",
        "> Generated by `tools/ace-code_audit0.8.0.py --write`; do not edit by hand.",
        "",
        f"ACE is a Cargo workspace of {len(list(ROOT.glob('crates/*')))} crates. The **public API** is the",
        f"{', '.join(f'`{c}`' for c in PUBLIC_CRATES)} crate (and the `ace` command-line tool); every",
        "other crate is an **internal building block** (`publish = false`) whose `pub` items exist for",
        "the other ACE crates, the benchmark harness and the fuzz targets and may change in any release.",
        "ACE 0.6.0 removed the `#[deprecated]` entry points of 0.5.1 and narrowed nothing else.",
        "`ace-ffi` exports the C ABI (`include/ace.h`, `docs/FFI.md`); its Rust items are internal and",
        "its `extern \"C\"` functions are the interface.",
        "",
        "## The facade and what it is built on",
        "",
        "| `ace` item | Built on |",
        "|---|---|",
    ]
    lines += [f"| `{new}` | {old} |" for new, old in FACADE_06]
    lines += ["", "## Crates", "", "| Crate | Kind | Items | Methods | Deprecated |", "|---|---|---|---|---|"]
    per_crate = []
    for crate in sorted(ROOT.glob("crates/*")):
        if not (crate / "src" / "lib.rs").exists():
            continue
        items = public_items(crate)
        kind = "public" if crate.name in PUBLIC_CRATES else "internal"
        top = [i for i in items if not i[4]]
        methods = [i for i in items if i[4]]
        lines.append(f"| `{crate.name}` | {kind} | {len(top)} | {len(methods)} | "
                     f"{sum(1 for i in items if i[5])} |")
        per_crate.append((crate.name, kind, items))
    for name, kind, items in per_crate:
        lines += ["", f"### `{name}` ({kind})", "", "| Item | Kind | Location | Summary |", "|---|---|---|---|"]
        for file, item, number, doc, is_method, deprecated in items:
            label = "method" if is_method else "item"
            if deprecated:
                label += ", **deprecated**"
            summary = doc.replace("|", "\\|")
            lines.append(f"| `{item}` | {label} | `{file.split('/src/', 1)[1]}:{number}` | {summary} |")
    return "\n".join(lines) + "\n"


def render(allows: list[tuple[str, int, str, str]], unsafe_sites: list[tuple[str, int, str]],
           ffi_rows: list[tuple[str, str, bool, bool]]) -> str:
    """Markdown of the generated panic audit."""
    panic_allows = [site for site in allows if any(lint in site[2] for lint in PANIC_LINTS)]
    lines = [
        "# ACE panic audit",
        "",
        "> Generated by `tools/ace-code_audit0.8.0.py --write`; do not edit by hand.",
        "",
        "## Policy",
        "",
        "`clippy::unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `unreachable` and",
        "`dbg_macro` are",
        "`deny` for every production crate (`[workspace.lints.clippy]`). Tests are exempt",
        "(`clippy.toml`: `allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-panic-in-tests`;",
        "integration tests carry an explicit file-level `allow`). The benchmark harness",
        "(`ace-bench`) is a measurement tool and may assert.",
        "",
        "## Panic-lint exceptions in production code",
        "",
        f"**{len(panic_allows)}** exception(s).",
        "",
    ]
    if panic_allows:
        lines += ["| Location | Lints | Justification |", "|---|---|---|"]
        lines += [f"| `{f}:{n}` | `{l}` | {j} |" for f, n, l, j in panic_allows]
        lines.append("")
    lines += ["## Other clippy allowances in production code", "",
              "| Location | Lints | Justification |", "|---|---|---|"]
    lines += [f"| `{f}:{n}` | `{l}` | {j} |" for f, n, l, j in allows if (f, n, l, j) not in panic_allows]
    lines += ["", "## `unsafe` inventory (details: `docs/UNSAFE-AUDIT.md`)", "",
              "| Location | Kind |", "|---|---|"]
    lines += [f"| `{f}:{n}` | {k} |" for f, n, k in unsafe_sites]
    lines += ["", "## C ABI entry points (`crates/ace-ffi`)", "",
              "No Rust panic may unwind into the host (`docs/NATIVE-INTEROP.md`, rule 7): every exported",
              "function runs under `guard` / `guard_value` except the infallible accessors.", "",
              f"**{len(ffi_rows)}** exported functions, "
              f"**{sum(1 for r in ffi_rows if r[2])}** guarded, "
              f"**{sum(1 for r in ffi_rows if not r[2])}** infallible accessors, "
              f"**{sum(1 for r in ffi_rows if r[3])}** called by a test.", "",
              "| Function | File | Panic guard | Tested |", "|---|---|---|---|"]
    lines += [f"| `{n}` | `{f}` | {'yes' if g else 'no (infallible)'} | {'yes' if t else 'no'} |"
              for n, f, g, t in ffi_rows]
    return "\n".join(lines) + "\n"


def audit_public_api(problems: list[str]) -> None:
    """Every public item of the API crates is documented; nothing is ``#[deprecated]``."""
    for crate in sorted(ROOT.glob("crates/*")):
        if not (crate / "src").is_dir():
            continue
        for file, item, number, doc, _method, deprecated in public_items(crate):
            if deprecated:
                problems.append(f"{file}:{number}: `{item}` is #[deprecated]; remove it (0.6 removed them)")
            elif crate.name in PUBLIC_CRATES and not doc:
                problems.append(f"{file}:{number}: public item `{item}` has no doc comment")


#: Exported functions that cannot fail and therefore are not wrapped in a panic guard: a guard
#: would overwrite the last-error message they report.
FFI_INFALLIBLE = {
    "ace_abi_version", "ace_engine_version",
    "ace_last_error_ptr", "ace_last_error_len", "ace_last_error_block",
}
FFI_FUZZ_TARGETS = ("ffi_decode", "ffi_handles")


def ffi_entry_points() -> list[tuple[str, str, bool]]:
    """Exported C functions of ``ace-ffi`` as ``(name, file, guarded)``."""
    found = []
    for path in sorted((ROOT / "crates" / "ace-ffi" / "src").glob("*.rs")):
        lines = path.read_text().splitlines()
        for index, line in enumerate(lines):
            if line.strip() != "#[no_mangle]":
                continue
            signature = lines[index + 1]
            name = signature.split("fn ", 1)[1].split("(", 1)[0].strip()
            end = index + 1
            while end < len(lines) and lines[end] != "}":
                end += 1
            body = "\n".join(lines[index + 1:end])
            guarded = "guard(" in body or "guard_value(" in body
            found.append((name, path.relative_to(ROOT).as_posix(), guarded))
    return found


def audit_ffi(problems: list[str]) -> list[tuple[str, str, bool, bool]]:
    """Checks the C ABI crate; returns ``(name, file, guarded, tested)`` per entry point."""
    header = (ROOT / "include" / "ace.h").read_text()
    tests = "\n".join(p.read_text() for p in sorted((ROOT / "crates" / "ace-ffi" / "tests").glob("*.rs")))
    tests += (ROOT / "crates" / "ace-ffi" / "src" / "fuzzing.rs").read_text()
    rows = []
    for name, file, guarded in ffi_entry_points():
        if not guarded and name not in FFI_INFALLIBLE:
            problems.append(f"{file}: `{name}` is exported without guard()/guard_value()")
        if re.search(rf"\b{name}\(", header) is None:
            problems.append(f"include/ace.h: `{name}` is not declared")
        tested = re.search(rf"\b{name}\b", tests) is not None
        if not tested:
            problems.append(f"crates/ace-ffi/tests: `{name}` is never called by a test")
        rows.append((name, file, guarded, tested))
    cargo = (ROOT / "fuzz" / "Cargo.toml").read_text()
    for target in FFI_FUZZ_TARGETS:
        if not (ROOT / "fuzz" / "fuzz_targets" / f"{target}.rs").exists() or f'name = "{target}"' not in cargo:
            problems.append(f"fuzz: target `{target}` is missing")
    return rows


def main(argv: list[str]) -> int:
    """Run the audit; with ``--write`` regenerate the Markdown report."""
    problems: list[str] = []
    unsafe_sites = audit_unsafe(problems)
    allows = audit_allows(problems)
    audit_test_layout(problems)
    audit_decoder(problems)
    audit_public_api(problems)
    ffi_rows = audit_ffi(problems)
    if "--write" in argv:
        target = ROOT / "docs" / "PANIC-AUDIT.md"
        target.write_text(render(allows, unsafe_sites, ffi_rows))
        print(f"written {target.relative_to(ROOT)}")
        target = ROOT / "docs" / "DECODER-AUDIT.md"
        target.write_text(render_decoder(allows))
        print(f"written {target.relative_to(ROOT)}")
        target = ROOT / "docs" / "API-INVENTORY.md"
        target.write_text(render_api())
        print(f"written {target.relative_to(ROOT)}")
    print(f"unsafe sites: {len(unsafe_sites)} (all in audited files: {not any('outside' in p for p in problems)})")
    print(f"production clippy allowances: {len(allows)}")
    for problem in problems:
        print(f"AUDIT: {problem}", file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
