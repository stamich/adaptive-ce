#!/usr/bin/env python3
"""Deterministic release packaging of ACE 0.5.1.

* collects the source tree minus build/output artefacts (``EXCLUDED_*``);
* audits script names (``ace-`` prefix + ``0.5.1``) and fails on stale versioned files;
* writes ``ACE-0.5.1/MANIFEST.txt`` and ``ACE-0.5.1/SHA256SUMS`` into the archive;
* creates a ZIP with sorted entries, fixed timestamps (1980-01-01), 0644/0755 modes and no
  user metadata, so two runs produce the same SHA-256 (``--verify-reproducible`` checks it).

With ``--dist DIR --results DIR`` it also writes the release distribution (0.5.1):

* ``ace-<version>-source.zip``      -- the deterministic source archive above;
* ``ace-<version>-benchmarks.zip``  -- every result document of the release run (deterministic);
* ``RELEASE-REPORT.md``             -- git revision, toolchain, host, every gate verdict and the
  documents behind them;
* ``MANIFEST.sha256``               -- SHA-256 of the three files above.

usage: ace-package0.5.1.py [--output DIR] [--verify-reproducible] [--dist DIR --results DIR]
"""
from __future__ import annotations

import argparse
import hashlib
import io
import pathlib
import re
import sys
import zipfile

VERSION = "0.5.1"
ROOT = pathlib.Path(__file__).resolve().parent.parent
PREFIX = f"ACE-{VERSION}"
FIXED_TIME = (1980, 1, 1, 0, 0, 0)

#: Directory names never packaged, wherever they appear.
EXCLUDED_DIRS = {"target", "__pycache__", ".git", ".idea", ".vscode", "artifacts", "dist"}
#: Paths (relative, POSIX) never packaged.
EXCLUDED_PATHS = {"examples/corpus", "fuzz/corpus"}
#: File-name patterns never packaged.
EXCLUDED_FILES = [re.compile(p) for p in (r"^\.DS_Store$", r"\.pyc$", r"~$", r"\.swp$")]
#: Generated results never packaged (only examples/results/README.md is kept).
EXCLUDED_RESULTS = re.compile(r"^examples/results/(?!README\.md$)")
#: Scripts and tools must carry the product prefix and version in their names.
NAMED_GLOBS = ("*.sh", "demo/*.sh", "tools/*.py", "tools/*.sh")
#: Versioned files of an older milestone must not survive in the tree.
STALE = re.compile(r"(0\.4\.\d|0\.4-buildfix\d|0\.5\.0)")


def is_excluded(relative: pathlib.PurePosixPath) -> bool:
    """Whether ``relative`` must not be packaged."""
    text = relative.as_posix()
    if any(part in EXCLUDED_DIRS for part in relative.parts):
        return True
    if any(text == path or text.startswith(path + "/") for path in EXCLUDED_PATHS):
        return True
    if EXCLUDED_RESULTS.match(text):
        return True
    return any(pattern.search(relative.name) for pattern in EXCLUDED_FILES)


def collect() -> list[pathlib.PurePosixPath]:
    """Sorted list of packaged files (relative POSIX paths)."""
    files = []
    for path in ROOT.rglob("*"):
        relative = pathlib.PurePosixPath(path.relative_to(ROOT).as_posix())
        if path.is_file() and not is_excluded(relative):
            files.append(relative)
    return sorted(files)


def audit(files: list[pathlib.PurePosixPath]) -> list[str]:
    """Naming and stale-file problems (empty list = clean)."""
    problems = []
    for pattern in NAMED_GLOBS:
        for path in ROOT.glob(pattern):
            name = path.name
            if not name.startswith("ace-") or VERSION not in name:
                problems.append(f"script name lacks 'ace-' prefix or '{VERSION}': {path.relative_to(ROOT)}")
    for relative in files:
        top = relative.parts[0]
        if top in ("tools", "demo") or len(relative.parts) == 1:
            if STALE.search(relative.name):
                problems.append(f"stale versioned file: {relative}")
    return problems


def file_mode(path: pathlib.Path) -> int:
    """0755 for executables, 0644 otherwise."""
    return 0o755 if path.stat().st_mode & 0o111 else 0o644


def build_zip(files: list[pathlib.PurePosixPath]) -> bytes:
    """Deterministic ZIP image of ``files`` plus generated MANIFEST.txt and SHA256SUMS."""
    digests = {f: hashlib.sha256((ROOT / f).read_bytes()).hexdigest() for f in files}
    manifest = "".join(f"{f}\n" for f in files)
    sums = "".join(f"{digests[f]}  {f}\n" for f in files)
    entries = [(str(f), (ROOT / f).read_bytes(), file_mode(ROOT / f)) for f in files]
    entries += [("MANIFEST.txt", manifest.encode(), 0o644), ("SHA256SUMS", sums.encode(), 0o644)]
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as archive:
        for name, data, mode in sorted(entries):
            info = zipfile.ZipInfo(f"{PREFIX}/{name}", date_time=FIXED_TIME)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 3
            info.external_attr = (0o100000 | mode) << 16
            archive.writestr(info, data, compresslevel=9)
    return buffer.getvalue()


def deterministic_zip(entries: list[tuple[str, bytes]]) -> bytes:
    """ZIP image of ``(name, data)`` entries with fixed timestamps and modes."""
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as archive:
        for name, data in sorted(entries):
            info = zipfile.ZipInfo(name, date_time=FIXED_TIME)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 3
            info.external_attr = (0o100000 | 0o644) << 16
            archive.writestr(info, data, compresslevel=9)
    return buffer.getvalue()


def command_output(*command: str) -> str:
    """First line of a command's output, or ``unknown``."""
    import subprocess
    try:
        result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=False)
    except OSError:
        return "unknown"
    lines = result.stdout.strip().splitlines()
    return lines[0] if result.returncode == 0 and lines else "unknown"


def load_json(path: pathlib.Path) -> dict | None:
    """A JSON document, or ``None`` when absent / unreadable."""
    import json
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return None


def gate_summary(doc: dict | None) -> tuple[str, str]:
    """``(status, counts)`` of a gate document (Regression V3, repeatability, A/B)."""
    if doc is None:
        return "missing", "-"
    checks = [check for row in doc.get("workloads", []) for check in row.get("checks", [])]
    counts: dict[str, int] = {}
    for check in checks:
        counts[check.get("status", "?")] = counts.get(check.get("status", "?"), 0) + 1
    return str(doc.get("status", "?")), ", ".join(f"{k} {v}" for k, v in sorted(counts.items())) or "-"


def release_report(results: pathlib.Path, source_sha: str, bench_names: list[str]) -> str:
    """Markdown release report."""
    cpu = "unknown"
    try:
        cpu = next((line.split(":", 1)[1].strip() for line in
                    pathlib.Path("/proc/cpuinfo").read_text().splitlines()
                    if line.startswith("model name")), "unknown")
    except OSError:
        pass
    dirty = command_output("git", "status", "--porcelain", "--untracked-files=no")
    revision = command_output("git", "rev-parse", "HEAD")
    lines = [
        f"# ACE {VERSION} — release report",
        "",
        "> Generated by `ace-release0.5.1.sh` (`tools/ace-package0.5.1.py --dist`).",
        "",
        "| Item | Value |",
        "|---|---|",
        f"| version | {VERSION} |",
        f"| git revision | `{revision}`{' (modified tree)' if dirty not in ('unknown', '') else ''} |",
        f"| rustc | {command_output('rustc', '-V')} |",
        f"| cargo | {command_output('cargo', '-V')} |",
        f"| host | {command_output('uname', '-srm')} — {cpu} |",
        f"| source archive SHA-256 | `{source_sha}` |",
        "",
        "## Gates",
        "",
        "| Gate document | Status | Checks |",
        "|---|---|---|",
    ]
    for label, name in (("Regression V3", f"benchmark-{VERSION}-regression.json"),
                        ("Repeatability (3 runs)", f"benchmark-{VERSION}-repeatability.json")):
        status, counts = gate_summary(load_json(results / name))
        lines.append(f"| {label} (`{name}`) | **{status}** | {counts} |")
    for ab in sorted(results.glob(f"ab-{VERSION}-vs-*.json")):
        status, counts = gate_summary(load_json(ab))
        lines.append(f"| Interleaved A/B (`{ab.name}`) | **{status}** | {counts} |")
    checklist = results / f"release-checklist-{VERSION}.md"
    if checklist.exists():
        lines += ["", "## Checklist", ""]
        lines += [line for line in checklist.read_text().splitlines()
                  if line.startswith("|") or line.startswith("Mode:")]
    lines += ["", "## Documents in `ace-" + VERSION + "-benchmarks.zip`", ""]
    lines += [f"* `{name}`" for name in bench_names]
    return "\n".join(lines) + "\n"


def write_dist(dist: pathlib.Path, results: pathlib.Path, image: bytes) -> None:
    """Source archive, benchmark archive, release report and manifest."""
    dist.mkdir(parents=True, exist_ok=True)
    source = dist / f"ace-{VERSION}-source.zip"
    source.write_bytes(image)
    entries = [(path.relative_to(results).as_posix(), path.read_bytes())
               for path in sorted(results.rglob("*"))
               if path.is_file() and path.suffix in (".json", ".md")]
    bench = dist / f"ace-{VERSION}-benchmarks.zip"
    bench.write_bytes(deterministic_zip(entries))
    report = dist / "RELEASE-REPORT.md"
    report.write_text(release_report(results, hashlib.sha256(image).hexdigest(),
                                     [name for name, _ in entries]))
    manifest = "".join(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n"
                       for path in (source, bench, report))
    (dist / "MANIFEST.sha256").write_text(manifest)
    print(f"distribution written to {dist}")


def main(argv: list[str]) -> int:
    """Audit, package and optionally verify reproducibility."""
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--output", type=pathlib.Path, default=ROOT / "target" / "package")
    parser.add_argument("--verify-reproducible", action="store_true")
    parser.add_argument("--dist", type=pathlib.Path)
    parser.add_argument("--results", type=pathlib.Path)
    args = parser.parse_args(argv)
    if args.dist and not args.results:
        parser.error("--dist requires --results")

    files = collect()
    problems = audit(files)
    if problems:
        for problem in problems:
            print(f"AUDIT: {problem}", file=sys.stderr)
        return 1
    image = build_zip(files)
    digest = hashlib.sha256(image).hexdigest()
    if args.verify_reproducible:
        second = hashlib.sha256(build_zip(collect())).hexdigest()
        if second != digest:
            print(f"NOT REPRODUCIBLE: {digest} != {second}", file=sys.stderr)
            return 1
        print("reproducible: two builds produced the same SHA-256")
    args.output.mkdir(parents=True, exist_ok=True)
    archive = args.output / f"{PREFIX}.zip"
    archive.write_bytes(image)
    (args.output / f"{PREFIX}.zip.sha256").write_text(f"{digest}  {archive.name}\n")
    print(f"{archive} ({len(files)} files, {len(image):,} bytes)\nsha256 {digest}")
    if args.dist:
        write_dist(args.dist, args.results, image)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
