package io.ace;

import io.ace.ffi.AceBindings;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.util.Arrays;
import java.util.List;
import java.util.Objects;

/**
 * A preset dictionary: bytes that every block is compressed "after", so small inputs that share a
 * vocabulary (log lines, JSON records, protocol messages) compress much better. Immutable.
 *
 * <p>A dictionary is used in one of two ways:
 *
 * <ul>
 *   <li>{@link Config.Builder#dictionary(Dictionary)} embeds it in the file when that pays, so the
 *       file is self-contained;
 *   <li>{@link Config.Builder#externalDictionary(Dictionary)} stores only its {@link #id()}. Every
 *       reader must then be created with the same dictionary, see {@link
 *       Engine#create(Config, DecodeLimits, java.util.Collection)}; otherwise decoding fails with
 *       {@link Status#UNSUPPORTED}.
 * </ul>
 *
 * <p>Train one with {@link #train(List, int)} from typical inputs, or wrap bytes you already have
 * with {@link #of(byte[])}. Training is deterministic: the same samples in the same order give the
 * same dictionary on every machine. See {@code docs/DICTIONARIES.md}.
 *
 * <pre>{@code
 * Dictionary dictionary = Dictionary.train(samples, 4096);
 * Config config = Config.builder().externalDictionary(dictionary).build();
 * try (Engine writer = Engine.create(config);
 *      Engine reader = Engine.create(Config.defaults(), DecodeLimits.defaults(), List.of(dictionary))) {
 *     byte[] packed = writer.compress(record);
 *     byte[] same = reader.decompress(packed);
 * }
 * }</pre>
 */
public final class Dictionary {
    /** Largest dictionary the format allows, in bytes (the LZ window). */
    public static final int MAX_SIZE = 65_535;

    private final long id;
    private final byte[] bytes;

    private Dictionary(long id, byte[] bytes) {
        this.id = id;
        this.bytes = bytes;
    }

    /**
     * A dictionary with the given content. Its {@link #id()} is derived from the bytes, so two
     * dictionaries with the same bytes are the same dictionary.
     *
     * @param bytes 1 to {@link #MAX_SIZE} bytes
     * @throws AceException with {@link Status#INVALID_ARGUMENT} when the size is out of range
     */
    public static Dictionary of(byte[] bytes) {
        return create(bytes, 0);
    }

    /**
     * A dictionary with an identifier assigned by the caller, for systems that already number
     * their dictionaries. Files refer to the dictionary by this id.
     *
     * @param id a non-zero identifier
     * @param bytes 1 to {@link #MAX_SIZE} bytes
     * @throws IllegalArgumentException when {@code id} is 0
     * @throws AceException with {@link Status#INVALID_ARGUMENT} when the size is out of range
     */
    public static Dictionary of(long id, byte[] bytes) {
        if (id == 0) {
            throw new IllegalArgumentException("id 0 is reserved for derived identifiers; use of(bytes)");
        }
        return create(bytes, id);
    }

    private static Dictionary create(byte[] bytes, long id) {
        Objects.requireNonNull(bytes, "bytes");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = Native.slot(arena);
            Native.check(Native.fn().aceDictionaryNew(Native.copyIn(bytes, arena), bytes.length, id, out));
            return read(Native.pointerIn(out));
        }
    }

    /**
     * Trains a dictionary of about {@code size} bytes from sample inputs.
     *
     * <p>100 to 1 000 typical inputs of one data source are usually enough; at most 8 MiB of them
     * are read.
     *
     * @param samples at least two inputs
     * @param size target size, 64 to {@link #MAX_SIZE} bytes
     * @throws AceException with {@link Status#INVALID_ARGUMENT} for a size out of range, fewer than
     *     two samples, or samples that share no content
     */
    public static Dictionary train(List<byte[]> samples, int size) {
        Objects.requireNonNull(samples, "samples");
        try (Arena arena = Arena.ofConfined()) {
            int count = samples.size();
            MemorySegment pointers = arena.allocate(ValueLayout.ADDRESS, Math.max(1, count));
            MemorySegment lengths = arena.allocate(ValueLayout.JAVA_LONG, Math.max(1, count));
            for (int i = 0; i < count; i++) {
                byte[] sample = Objects.requireNonNull(samples.get(i), "sample");
                pointers.setAtIndex(ValueLayout.ADDRESS, i, Native.copyIn(sample, arena));
                lengths.setAtIndex(ValueLayout.JAVA_LONG, i, sample.length);
            }
            MemorySegment out = Native.slot(arena);
            Native.check(Native.fn().aceDictionaryTrain(pointers, lengths, count, size, out));
            return read(Native.pointerIn(out));
        }
    }

    /** Copies id and bytes out of a native handle and releases the handle. */
    private static Dictionary read(MemorySegment handle) {
        AceBindings.Functions f = Native.fn();
        try {
            long length = f.aceDictionaryLen(handle);
            byte[] copy = f.aceDictionaryData(handle).reinterpret(length).toArray(ValueLayout.JAVA_BYTE);
            return new Dictionary(f.aceDictionaryId(handle), copy);
        } finally {
            f.aceDictionaryFree(handle);
        }
    }

    /** The identifier written into files that use this dictionary. */
    public long id() {
        return id;
    }

    /** A copy of the dictionary bytes. Store them to give the dictionary to a reader later. */
    public byte[] bytes() {
        return bytes.clone();
    }

    /** Number of bytes. */
    public int size() {
        return bytes.length;
    }

    /**
     * Creates a native handle in {@code arena}'s scope and registers its release in {@code
     * temporaries}.
     */
    MemorySegment toNative(Arena arena, Temporaries temporaries) {
        MemorySegment out = Native.slot(arena);
        Native.check(Native.fn().aceDictionaryNew(Native.copyIn(bytes, arena), bytes.length, id, out));
        MemorySegment handle = Native.pointerIn(out);
        temporaries.onClose(() -> Native.fn().aceDictionaryFree(handle));
        return handle;
    }

    @Override
    public boolean equals(Object other) {
        return other instanceof Dictionary that && id == that.id && Arrays.equals(bytes, that.bytes);
    }

    @Override
    public int hashCode() {
        return Long.hashCode(id) * 31 + Arrays.hashCode(bytes);
    }

    @Override
    public String toString() {
        return "Dictionary[id=" + Long.toHexString(id) + ", size=" + bytes.length + "]";
    }
}
