package io.ace;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;

/**
 * Reads byte ranges of an indexed ACE file without decompressing the rest of it. Only the blocks
 * that overlap a range are read and decoded, so the cost depends on the range and the block size,
 * not on the file size.
 *
 * <p>A reader keeps a one-block cache and is not thread-safe: use one reader per thread, or
 * synchronise. Close it when done. Obtain one from {@link Engine#openRange(byte[])}.
 */
public final class RangeReader implements AutoCloseable {
    private final NativeHandle handle;
    private final long length;
    private final long blockCount;

    private RangeReader(MemorySegment pointer) {
        this.handle = new NativeHandle(this, "range reader", pointer, RangeReader::free);
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment value = arena.allocate(ValueLayout.JAVA_LONG);
            Native.check(Native.fn().aceRangeLen(pointer, value));
            this.length = value.get(ValueLayout.JAVA_LONG, 0);
            Native.check(Native.fn().aceRangeBlockCount(pointer, value));
            this.blockCount = value.get(ValueLayout.JAVA_LONG, 0);
        } catch (RuntimeException | Error e) {
            handle.close();
            throw e;
        }
    }

    static RangeReader of(MemorySegment pointer) {
        return new RangeReader(pointer);
    }

    private static void free(MemorySegment pointer) {
        Native.fn().aceRangeFree(pointer);
    }

    /** Size of the original file in bytes. */
    public long length() {
        return length;
    }

    /** Number of independent blocks. */
    public long blockCount() {
        return blockCount;
    }

    /**
     * The original bytes {@code [start, end)}.
     *
     * @throws AceException {@link Status#INVALID_ARGUMENT} for a reversed range or one past the end;
     *     the decoding statuses for a damaged block
     */
    public byte[] read(long start, long end) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = Native.slot(arena);
            MemorySegment reader = handle.enter();
            try {
                Native.check(Native.fn().aceRangeRead(reader, start, end, out));
            } finally {
                handle.exit();
            }
            return Native.takeBytes(Native.pointerIn(out));
        }
    }

    /** One whole block by number. */
    public byte[] readBlock(long block) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = Native.slot(arena);
            MemorySegment reader = handle.enter();
            try {
                Native.check(Native.fn().aceRangeReadBlock(reader, block, out));
            } finally {
                handle.exit();
            }
            return Native.takeBytes(Native.pointerIn(out));
        }
    }

    /** The work of the last {@link #read(long, long)} on this reader. */
    public RangeStats lastStats() {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment values = arena.allocate(3 * 8L);
            MemorySegment reader = handle.enter();
            try {
                Native.check(Native.fn().aceRangeLastStats(
                        reader, values, values.asSlice(8), values.asSlice(16)));
            } finally {
                handle.exit();
            }
            return new RangeStats(
                    values.get(ValueLayout.JAVA_LONG, 0),
                    values.get(ValueLayout.JAVA_LONG, 8),
                    values.get(ValueLayout.JAVA_LONG, 16));
        }
    }

    /** Releases the native reader; later calls do nothing. */
    @Override
    public void close() {
        handle.close();
    }

    /** True once {@link #close()} was called. */
    public boolean isClosed() {
        return handle.isClosed();
    }
}
