#!/usr/bin/env python3
"""Generates the Panama (java.lang.foreign) bindings from include/ace.h.

This is the jextract substitute of ACE 0.8.0: jextract needs a separate LLVM download, while this
script reads the one header the project controls (cbindgen output, regular enough to parse) and
writes ``jvm/ace-native/src/main/java/io/ace/ffi/AceBindings.java``. Nothing in the JVM modules
lists a native function by hand; they call the generated methods.

usage: ace-genbindings0.8.0.py [--write | --check]

  --write   regenerate the Java file
  --check   exit 1 when the committed file differs from what the header produces (CI)
"""
from __future__ import annotations

import hashlib
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
HEADER = ROOT / "include" / "ace.h"
OUTPUT = ROOT / "jvm" / "ace-native" / "src" / "main" / "java" / "io" / "ace" / "ffi" / "AceBindings.java"

KINDS = {
    "size_t": "LONG", "uint64_t": "LONG", "int64_t": "LONG",
    "uint32_t": "INT", "int32_t": "INT",
    "uint8_t": "BYTE", "int8_t": "BYTE",
}
LAYOUT = {"LONG": "ValueLayout.JAVA_LONG", "INT": "ValueLayout.JAVA_INT",
          "BYTE": "ValueLayout.JAVA_BYTE", "ADDRESS": "ValueLayout.ADDRESS"}
JAVA_TYPE = {"LONG": "long", "INT": "int", "BYTE": "byte", "ADDRESS": "MemorySegment", "VOID": "void"}
SIZE = {"LONG": 8, "INT": 4, "BYTE": 1, "ADDRESS": 8}


def camel(name: str) -> str:
    """``ace_range_read`` -> ``aceRangeRead``; ``AceReadFn`` -> ``aceReadFn``."""
    parts = name.split("_")
    head = parts[0][:1].lower() + parts[0][1:]
    return head + "".join(part[:1].upper() + part[1:] for part in parts[1:])


def snake_upper(name: str) -> str:
    """``AceReadFn`` -> ``ACE_READ_FN``."""
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).upper()


class Header:
    """The declarations of ``ace.h``."""

    def __init__(self, text: str) -> None:
        self.defines: list[tuple[str, str, str]] = []
        self.opaque: list[str] = []
        self.structs: list[tuple[str, list[tuple[str, str, int, str]], str]] = []
        self.callbacks: list[tuple[str, str, list[tuple[str, str]], str]] = []
        self.aliases: dict[str, str] = {}
        self.functions: list[tuple[str, str, list[tuple[str, str]], str]] = []
        self._parse(text)

    def kind(self, ctype: str) -> str:
        """``LONG`` / ``INT`` / ``BYTE`` / ``ADDRESS`` / ``VOID`` of a C type."""
        ctype = ctype.strip()
        if "*" in ctype or ctype in {name for name, *_ in self.callbacks}:
            return "ADDRESS"
        base = ctype.replace("const ", "").replace("struct ", "").strip()
        base = self.aliases.get(base, base)
        if base == "void":
            return "VOID"
        if base in KINDS:
            return KINDS[base]
        raise SystemExit(f"genbindings: unsupported C type `{ctype}`")

    @staticmethod
    def _split_arg(argument: str) -> tuple[str, str]:
        match = re.match(r"^(.*?)([A-Za-z_][A-Za-z0-9_]*)$", argument.strip())
        if not match or not match.group(1).strip():
            raise SystemExit(f"genbindings: cannot parse argument `{argument}`")
        return match.group(1).strip(), match.group(2)

    def _args(self, text: str) -> list[tuple[str, str]]:
        text = text.strip()
        if text in ("", "void"):
            return []
        return [self._split_arg(part) for part in text.split(",")]

    def _parse(self, text: str) -> None:
        lines = text.replace("\r", "").split("\n")
        doc: list[str] = []
        index = 0
        while index < len(lines):
            line = lines[index].strip()
            index += 1
            if line.startswith("//"):
                doc.append(line[2:].strip())
                continue
            comment = " ".join(d for d in doc if d).strip()
            if not line:
                doc = []
                continue
            match = re.match(r"#define\s+(ACE_\w+)\s+(.+)$", line)
            if match:
                self.defines.append((match.group(1), match.group(2).strip(), comment))
            elif re.match(r"typedef struct (\w+) \1;$", line):
                self.opaque.append(re.match(r"typedef struct (\w+)", line).group(1))
            elif re.match(r"typedef struct (\w+) \{$", line):
                name = re.match(r"typedef struct (\w+)", line).group(1)
                fields, field_doc = [], []
                while not lines[index].strip().startswith("}"):
                    field = lines[index].strip()
                    index += 1
                    if field.startswith("//"):
                        field_doc.append(field[2:].strip())
                        continue
                    match = re.match(r"^(\w+)\s+(\w+)(?:\[(\d+)\])?;$", field)
                    if not match:
                        raise SystemExit(f"genbindings: cannot parse field `{field}`")
                    fields.append((match.group(1), match.group(2), int(match.group(3) or 0), " ".join(field_doc)))
                    field_doc = []
                index += 1
                self.structs.append((name, fields, comment))
            elif re.match(r"typedef\s+(\w+)\s+\(\*(\w+)\)\((.*)\);$", line):
                match = re.match(r"typedef\s+(\w+)\s+\(\*(\w+)\)\((.*)\);$", line)
                self.callbacks.append((match.group(2), match.group(1), self._args(match.group(3)), comment))
            elif re.match(r"typedef\s+(\w+)\s+(\w+);$", line):
                match = re.match(r"typedef\s+(\w+)\s+(\w+);$", line)
                self.aliases[match.group(2)] = match.group(1)
            elif (line[0].isalpha() and "(" in line and not line.startswith(("typedef", "extern"))):
                while not line.endswith(";"):
                    line += " " + lines[index].strip()
                    index += 1
                match = re.match(r"^(.*?)\b(ace_\w+)\((.*)\);$", line)
                if not match:
                    raise SystemExit(f"genbindings: cannot parse function `{line}`")
                self.functions.append((match.group(2), match.group(1).strip(), self._args(match.group(3)), comment))
            doc = []


def layout_of(header: Header, fields: list[tuple[str, str, int, str]]):
    """Struct member layouts with explicit padding, as ``(java expr, name, offset, size)``."""
    members, offset, alignment = [], 0, 1
    for ctype, name, count, _doc in fields:
        kind = header.kind(ctype)
        size, align = SIZE[kind], SIZE[kind]
        alignment = max(alignment, align)
        padding = (-offset) % align
        if padding:
            members.append((f"MemoryLayout.paddingLayout({padding})", None, offset, padding))
            offset += padding
        expression = LAYOUT[kind]
        if count:
            expression = f"MemoryLayout.sequenceLayout({count}, {expression})"
        members.append((f'{expression}.withName("{name}")', name, offset, size * max(count, 1)))
        offset += size * max(count, 1)
    tail = (-offset) % alignment
    if tail:
        members.append((f"MemoryLayout.paddingLayout({tail})", None, offset, tail))
        offset += tail
    return members, offset


def constant_value(value: str) -> tuple[str, str]:
    """Java type and literal of a ``#define`` value."""
    if value == "UINT64_MAX":
        return "long", "-1L"
    if re.fullmatch(r"-?\d+", value):
        return "int", value
    raise SystemExit(f"genbindings: unsupported constant `{value}`")


def generate(header: Header, digest: str) -> str:
    out: list[str] = []
    w = out.append
    w("// GENERATED by tools/ace-genbindings0.8.0.py from include/ace.h. Do not edit.")
    w(f"// Source digest (SHA-256 of include/ace.h): {digest}")
    w("package io.ace.ffi;")
    w("")
    w("import java.lang.foreign.Arena;")
    w("import java.lang.foreign.FunctionDescriptor;")
    w("import java.lang.foreign.Linker;")
    w("import java.lang.foreign.MemoryLayout;")
    w("import java.lang.foreign.MemorySegment;")
    w("import java.lang.foreign.StructLayout;")
    w("import java.lang.foreign.SymbolLookup;")
    w("import java.lang.foreign.ValueLayout;")
    w("import java.lang.invoke.MethodHandle;")
    w("import java.lang.invoke.MethodHandles;")
    w("import java.lang.invoke.MethodType;")
    w("")
    w("/**")
    w(" * Panama bindings of the ACE C ABI (docs/FFI.md), one method per function of {@code ace.h}.")
    w(" *")
    w(" * <p>Create an instance with {@link #bind(SymbolLookup)}; it checks {@code ace_abi_version()}")
    w(" * against {@link #ABI_VERSION} before it resolves any other symbol.")
    w(" */")
    w("@SuppressWarnings({\"checkstyle:all\", \"unused\"})")
    w("public final class AceBindings {")
    w("    private AceBindings() {")
    w("        throw new AssertionError();")
    w("    }")
    w("")
    w("    /** Raised by {@link #bind(SymbolLookup)} when the library has another ABI version. */")
    w("    public static final class AbiMismatchException extends RuntimeException {")
    w("        private static final long serialVersionUID = 1L;")
    w("")
    w("        /** @param message names the expected and the found version */")
    w("        public AbiMismatchException(String message) {")
    w("            super(message);")
    w("        }")
    w("    }")
    w("")
    w("    private static final Linker LINKER = Linker.nativeLinker();")
    w("")
    w("    static {")
    w("        if (ValueLayout.ADDRESS.byteSize() != 8) {")
    w("            throw new UnsupportedOperationException(\"the ACE bindings need a 64-bit platform (size_t = 8 bytes)\");")
    w("        }")
    w("    }")
    w("")
    w("    // ---- constants ----")
    for name, value, doc in header.defines:
        jtype, literal = constant_value(value)
        if doc:
            w(f"    /** {doc} */")
        short = "ABI_VERSION" if name == "ACE_ABI_VERSION" else name
        w(f"    public static final {jtype} {short} = {literal};")
    w("")
    for name, fields, doc in header.structs:
        members, size = layout_of(header, fields)
        const = snake_upper(name)
        w(f"    // ---- struct {name} ----")
        w(f"    /** Layout of {{@code {name}}} ({size} bytes). */")
        w(f"    public static final StructLayout {const} = MemoryLayout.structLayout(")
        w(",\n".join(f"            {expr}" for expr, *_ in members))
        w(f'    ).withName("{name}");')
        w(f"    /** Size of {{@code {name}}} in bytes. */")
        w(f"    public static final long {const}_SIZE = {size}L;")
        for _expr, member, offset, _bytes in members:
            if member:
                w(f"    public static final long {const}_{member.upper()} = {offset}L;")
        w("")
    for name, ret, args, doc in header.callbacks:
        iface = name
        kinds = [header.kind(t) for t, _ in args]
        rk = header.kind(ret)
        params = ", ".join(f"{JAVA_TYPE[k]} {n}" for k, (_, n) in zip(kinds, args))
        descriptor_args = ", ".join(LAYOUT[k] for k in kinds)
        descriptor = (f"FunctionDescriptor.of({LAYOUT[rk]}{', ' if kinds else ''}{descriptor_args})")
        w(f"    // ---- callback {name} ----")
        w(f"    /** {doc} */")
        w(f"    @FunctionalInterface")
        w(f"    public interface {iface} {{")
        w(f"        /** Called by the library; must not throw. */")
        w(f"        {JAVA_TYPE[rk]} apply({params});")
        w(f"    }}")
        w(f"    /** Native signature of {{@link {iface}}}. */")
        w(f"    public static final FunctionDescriptor {snake_upper(name)}_DESCRIPTOR = {descriptor};")
        mt = ", ".join(f"{JAVA_TYPE[k]}.class" for k in kinds)
        mt = mt.replace("MemorySegment.class", "MemorySegment.class")
        w(f"    /** Creates a native function pointer that calls {{@code function}}; it lives as long as {{@code arena}}. */")
        w(f"    public static MemorySegment {camel(name)}Stub({iface} function, Arena arena) {{")
        w(f"        try {{")
        w(f"            MethodHandle handle = MethodHandles.lookup().findVirtual({iface}.class, \"apply\",")
        w(f"                    MethodType.methodType({JAVA_TYPE[rk]}.class{', ' if kinds else ''}{mt})).bindTo(function);")
        w(f"            return LINKER.upcallStub(handle, {snake_upper(name)}_DESCRIPTOR, arena);")
        w(f"        }} catch (ReflectiveOperationException e) {{")
        w(f"            throw new AssertionError(e);")
        w(f"        }}")
        w(f"    }}")
        w("")
    w("    // ---- functions ----")
    w("    /** The resolved functions of one library. */")
    w("    public static final class Functions {")
    for name, ret, args, doc in header.functions:
        w(f"        private final MethodHandle {camel(name)};")
    w("")
    w("        private Functions(SymbolLookup lookup) {")
    for name, ret, args, doc in header.functions:
        kinds = [header.kind(t) for t, _ in args]
        rk = header.kind(ret)
        arg_layouts = ", ".join(LAYOUT[k] for k in kinds)
        if rk == "VOID":
            descriptor = f"FunctionDescriptor.ofVoid({arg_layouts})"
        else:
            descriptor = f"FunctionDescriptor.of({LAYOUT[rk]}{', ' if kinds else ''}{arg_layouts})"
        w(f"            this.{camel(name)} = downcall(lookup, \"{name}\", {descriptor});")
    w("        }")
    w("")
    for name, ret, args, doc in header.functions:
        kinds = [header.kind(t) for t, _ in args]
        rk = header.kind(ret)
        params = ", ".join(f"{JAVA_TYPE[k]} {n}" for k, (_, n) in zip(kinds, args))
        names = ", ".join(n for _, n in args)
        w(f"        /** {{@code {name}}}: {doc.split('# Safety')[0].strip()} */")
        w(f"        public {JAVA_TYPE[rk]} {camel(name)}({params}) {{")
        w(f"            try {{")
        call = f"this.{camel(name)}.invokeExact({names})"
        if rk == "VOID":
            w(f"                {call};")
        else:
            w(f"                return ({JAVA_TYPE[rk]}) {call};")
        w(f"            }} catch (Throwable e) {{")
        w(f"                throw new AssertionError(\"calling {name} failed\", e);")
        w(f"            }}")
        w(f"        }}")
        w("")
    w("    }")
    w("")
    w("    private static MethodHandle downcall(SymbolLookup lookup, String symbol, FunctionDescriptor descriptor) {")
    w("        MemorySegment address = lookup.find(symbol).orElseThrow(")
    w("                () -> new UnsatisfiedLinkError(\"the native library does not export \" + symbol));")
    w("        return LINKER.downcallHandle(address, descriptor);")
    w("    }")
    w("")
    w("    /**")
    w("     * Binds the library behind {@code lookup}: checks the ABI version first, then resolves every")
    w("     * function.")
    w("     *")
    w("     * @throws AbiMismatchException when the library reports another ABI version")
    w("     * @throws UnsatisfiedLinkError when a symbol is missing")
    w("     */")
    w("    public static Functions bind(SymbolLookup lookup) {")
    w("        MethodHandle version = downcall(lookup, \"ace_abi_version\", FunctionDescriptor.of(ValueLayout.JAVA_INT));")
    w("        int found;")
    w("        try {")
    w("            found = (int) version.invokeExact();")
    w("        } catch (Throwable e) {")
    w("            throw new AssertionError(\"calling ace_abi_version failed\", e);")
    w("        }")
    w("        if (found != ABI_VERSION) {")
    w("            throw new AbiMismatchException(\"ACE native library has ABI version \" + found")
    w("                    + \", these bindings were generated for ABI version \" + ABI_VERSION);")
    w("        }")
    w("        return new Functions(lookup);")
    w("    }")
    w("}")
    return "\n".join(out) + "\n"


def main(argv: list[str]) -> int:
    text = HEADER.read_text()
    digest = hashlib.sha256(text.encode()).hexdigest()
    generated = generate(Header(text), digest)
    if "--check" in argv:
        if not OUTPUT.exists() or OUTPUT.read_text() != generated:
            print(f"AUDIT: {OUTPUT.relative_to(ROOT)} is out of date: run tools/ace-genbindings0.8.0.py --write",
                  file=sys.stderr)
            return 1
        print(f"{OUTPUT.relative_to(ROOT)} matches include/ace.h")
        return 0
    if "--write" in argv:
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT.write_text(generated)
        print(f"written {OUTPUT.relative_to(ROOT)}")
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
