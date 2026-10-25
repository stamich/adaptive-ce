package io.ace;

import io.ace.ffi.AceBindings;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.Collection;
import java.util.List;
import java.util.Objects;

/**
 * A compression engine: lossless, deterministic, schema-free block compression with random
 * access. The output is byte-identical to the Rust library and to every other ACE binding for the
 * same {@link Config} and input, whatever the thread count.
 *
 * <p>An engine is immutable and safe to share between threads. It owns native memory: close it
 * (try-with-resources) when done. A forgotten engine is released by the garbage collector
 * eventually, but not promptly.
 *
 * <p>The {@code byte[]} methods copy the data into and out of native memory. The {@link
 * MemorySegment} methods are zero-copy; their input must be a native segment (not backed by a Java
 * array) and results are {@link NativeBuffer}s that the caller closes.
 *
 * <pre>{@code
 * try (Engine engine = Engine.create()) {
 *     byte[] packed = engine.compress(data);
 *     byte[] same = engine.decompress(packed);
 * }
 * }</pre>
 *
 * <p>The native library is loaded on first use; see {@link NativeLibrary}.
 */
public final class Engine implements AutoCloseable {
    private final NativeHandle handle;
    private final Config config;
    private final DecodeLimits limits;

    private Engine(Config config, DecodeLimits limits, MemorySegment pointer) {
        this.config = config;
        this.limits = limits;
        this.handle = new NativeHandle(this, "engine", pointer, Engine::free);
    }

    private static void free(MemorySegment pointer) {
        Native.fn().aceEngineFree(pointer);
    }

    /** An engine with the default {@link Config} and {@link DecodeLimits}. */
    public static Engine create() {
        return create(Config.defaults(), DecodeLimits.defaults());
    }

    /** An engine with the given settings and the default decode limits. */
    public static Engine create(Config config) {
        return create(config, DecodeLimits.defaults());
    }

    /**
     * An engine with the given settings and decode limits.
     *
     * @throws AceException with {@link Status#INVALID_ARGUMENT} when the native library rejects the settings
     */
    public static Engine create(Config config, DecodeLimits limits) {
        return create(config, limits, List.of());
    }

    /**
     * An engine that also knows external dictionaries.
     *
     * <p>A file written with {@link Config.Builder#externalDictionary(Dictionary)} names its
     * dictionary by id only. Reading it needs a dictionary with that id, which this engine looks
     * up among {@code dictionaries}; a missing one fails with {@link Status#UNSUPPORTED} and the
     * message names the id. Dictionaries embedded in a file need nothing, and the dictionary of
     * {@code config} is always available to its own engine. Applies to every reading method,
     * including {@link #openRange(byte[])}.
     *
     * @throws AceException with {@link Status#INVALID_ARGUMENT} when the native library rejects the settings
     */
    public static Engine create(Config config, DecodeLimits limits, Collection<Dictionary> dictionaries) {
        Objects.requireNonNull(config, "config");
        Objects.requireNonNull(limits, "limits");
        Objects.requireNonNull(dictionaries, "dictionaries");
        try (Arena arena = Arena.ofConfined(); Temporaries temporaries = new Temporaries()) {
            MemorySegment nativeConfig = config.toNative(arena, temporaries);
            MemorySegment array = arena.allocate(ValueLayout.ADDRESS, Math.max(1, dictionaries.size()));
            int index = 0;
            for (Dictionary dictionary : dictionaries) {
                Objects.requireNonNull(dictionary, "dictionary");
                array.setAtIndex(ValueLayout.ADDRESS, index++, dictionary.toNative(arena, temporaries));
            }
            MemorySegment out = Native.slot(arena);
            Native.check(Native.fn().aceEngineNewEx(
                    nativeConfig, limits.toNative(arena), array, dictionaries.size(), out));
            return new Engine(config, limits, Native.pointerIn(out));
        }
    }

    /** The settings this engine was created with. */
    public Config config() {
        return config;
    }

    /** The decode limits this engine was created with. */
    public DecodeLimits limits() {
        return limits;
    }

    // ---- one-shot, byte[] ----

    /** Compresses {@code input} into an ACE file. */
    public byte[] compress(byte[] input) {
        Objects.requireNonNull(input, "input");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = Native.slot(arena);
            call(engine -> Native.fn().aceCompress(
                    engine, Native.copyIn(input, arena), input.length, out, MemorySegment.NULL));
            return Native.takeBytes(Native.pointerIn(out));
        }
    }

    /** Compresses {@code input} and reports what the run did. */
    public Compressed compressWithStats(byte[] input) {
        Objects.requireNonNull(input, "input");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = Native.slot(arena);
            MemorySegment stats = Native.slot(arena);
            call(engine -> Native.fn().aceCompress(
                    engine, Native.copyIn(input, arena), input.length, out, stats));
            byte[] statsJson = Native.takeBytes(Native.pointerIn(stats));
            byte[] file = Native.takeBytes(Native.pointerIn(out));
            return new Compressed(file, Stats.fromJson(new String(statsJson, StandardCharsets.UTF_8)));
        }
    }

    /**
     * Decompresses a complete ACE file under this engine's {@link DecodeLimits}.
     *
     * @throws AceException {@link Status#NOT_ACE_FILE}, {@link Status#CORRUPT}, {@link
     *     Status#CHECKSUM_MISMATCH}, {@link Status#UNSUPPORTED} or {@link Status#LIMIT_EXCEEDED}
     */
    public byte[] decompress(byte[] file) {
        Objects.requireNonNull(file, "file");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = Native.slot(arena);
            call(engine -> Native.fn().aceDecompress(engine, Native.copyIn(file, arena), file.length, out));
            return Native.takeBytes(Native.pointerIn(out));
        }
    }

    /** The planner's per-block report as JSON (docs/FFI.md, "Explain JSON"); much slower than compressing. */
    public String explainJson(byte[] input) {
        Objects.requireNonNull(input, "input");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = Native.slot(arena);
            call(engine -> Native.fn().aceExplainJson(engine, Native.copyIn(input, arena), input.length, out));
            return new String(Native.takeBytes(Native.pointerIn(out)), StandardCharsets.UTF_8);
        }
    }

    // ---- one-shot, zero-copy ----

    /** Compresses a native segment; the result is owned by the caller. */
    public NativeBuffer compress(MemorySegment input) {
        Objects.requireNonNull(input, "input");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = Native.slot(arena);
            call(engine -> Native.fn().aceCompress(engine, input, input.byteSize(), out, MemorySegment.NULL));
            return NativeBuffer.wrap(Native.pointerIn(out));
        }
    }

    /** Decompresses a native segment holding a complete ACE file; the result is owned by the caller. */
    public NativeBuffer decompress(MemorySegment file) {
        Objects.requireNonNull(file, "file");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = Native.slot(arena);
            call(engine -> Native.fn().aceDecompress(engine, file, file.byteSize(), out));
            return NativeBuffer.wrap(Native.pointerIn(out));
        }
    }

    /**
     * Decompresses into a caller-provided native segment and returns the number of bytes written.
     *
     * @throws BufferTooSmallException when {@code output} is smaller than the decoded data; nothing
     *     is written to it and {@link BufferTooSmallException#requiredSize()} gives the size
     */
    public long decompressInto(MemorySegment file, MemorySegment output) {
        Objects.requireNonNull(file, "file");
        Objects.requireNonNull(output, "output");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment written = arena.allocate(ValueLayout.JAVA_LONG);
            int status = callStatus(engine -> Native.fn().aceDecompressInto(
                    engine, file, file.byteSize(), output, output.byteSize(), written));
            long size = written.get(ValueLayout.JAVA_LONG, 0);
            if (status == AceBindings.ACE_ERR_BUFFER_TOO_SMALL) {
                AceException failure = Native.failure(status);
                throw new BufferTooSmallException(status, failure.getMessage(), size);
            }
            Native.check(status);
            return size;
        }
    }

    // ---- streaming ----

    /**
     * Compresses exactly {@code size} bytes read from {@code in} and writes the ACE file to {@code
     * out}. Needs {@link BlockSizePolicy#FIXED}. The file is assembled in memory and written when
     * the run has finished; the input is read block by block.
     *
     * @return statistics; only the input and output size, the format minor and the block count are filled
     * @throws IOException when {@code in} or {@code out} fails
     */
    public Stats compressStream(long size, InputStream in, OutputStream out) throws IOException {
        Objects.requireNonNull(in, "in");
        Objects.requireNonNull(out, "out");
        try (Arena arena = Arena.ofConfined()) {
            Callbacks callbacks = new Callbacks(in, out, arena);
            MemorySegment stats = Native.slot(arena);
            int status = callStatus(engine -> Native.fn().aceCompressStream(
                    engine, size, callbacks.readStub(), MemorySegment.NULL, callbacks.writeStub(),
                    MemorySegment.NULL, stats));
            callbacks.rethrow(status);
            Native.check(status);
            return Stats.fromJson(new String(Native.takeBytes(Native.pointerIn(stats)), StandardCharsets.UTF_8));
        }
    }

    /**
     * Decompresses an ACE file read from {@code in} and writes the original bytes to {@code out},
     * one block at a time.
     *
     * @throws IOException when {@code in} or {@code out} fails
     */
    public void decompressStream(InputStream in, OutputStream out) throws IOException {
        Objects.requireNonNull(in, "in");
        Objects.requireNonNull(out, "out");
        try (Arena arena = Arena.ofConfined()) {
            Callbacks callbacks = new Callbacks(in, out, arena);
            int status = callStatus(engine -> Native.fn().aceDecompressStream(
                    engine, callbacks.readStub(), MemorySegment.NULL, callbacks.writeStub(), MemorySegment.NULL));
            callbacks.rethrow(status);
            Native.check(status);
        }
    }

    // ---- random access ----

    /** Opens an indexed ACE file held in a Java array; the bytes are copied. */
    public RangeReader openRange(byte[] file) {
        Objects.requireNonNull(file, "file");
        try (Arena arena = Arena.ofConfined()) {
            return openRange(Native.copyIn(file, arena), file.length);
        }
    }

    /** Opens an indexed ACE file held in a native segment; the bytes are copied. */
    public RangeReader openRange(MemorySegment file) {
        Objects.requireNonNull(file, "file");
        return openRange(file, file.byteSize());
    }

    private RangeReader openRange(MemorySegment file, long length) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = Native.slot(arena);
            call(engine -> Native.fn().aceRangeOpen(engine, file, length, out));
            return RangeReader.of(Native.pointerIn(out));
        }
    }

    /** Opens an indexed ACE file on disk; blocks are read on demand. */
    public RangeReader openRange(Path file) {
        Objects.requireNonNull(file, "file");
        byte[] path = file.toString().getBytes(StandardCharsets.UTF_8);
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = Native.slot(arena);
            call(engine -> Native.fn().aceRangeOpenPath(engine, Native.copyIn(path, arena), path.length, out));
            return RangeReader.of(Native.pointerIn(out));
        }
    }

    // ---- lifetime ----

    /** Releases the native engine; later calls do nothing. Calls in flight finish first. */
    @Override
    public void close() {
        handle.close();
    }

    /** True once {@link #close()} was called. */
    public boolean isClosed() {
        return handle.isClosed();
    }

    // ---- internals ----

    private interface NativeCall {
        int run(MemorySegment engine);
    }

    private int callStatus(NativeCall call) {
        MemorySegment engine = handle.enter();
        try {
            return call.run(engine);
        } finally {
            handle.exit();
        }
    }

    private void call(NativeCall call) {
        Native.check(callStatus(call));
    }
}
