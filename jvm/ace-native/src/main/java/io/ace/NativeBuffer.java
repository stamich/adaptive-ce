package io.ace;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.ref.Cleaner;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Bytes owned by the native library, readable without a copy through {@link #segment()}.
 *
 * <p>The segment is valid until {@link #close()}; afterwards any access to it throws {@link
 * IllegalStateException}. Closing is idempotent. A buffer that is garbage collected without being
 * closed is released by a safety net, but relying on it holds native memory for longer than needed.
 */
public final class NativeBuffer implements AutoCloseable {
    private static final Cleaner CLEANER = Cleaner.create();

    private final Arena arena;
    private final MemorySegment segment;
    private final Cleaner.Cleanable cleanable;

    /** Closes the arena, which runs the native release; must not reference the buffer. */
    private record Release(Arena arena) implements Runnable {
        @Override
        public void run() {
            try {
                arena.close();
            } catch (IllegalStateException alreadyClosed) {
                // closed explicitly before the cleaner ran
            }
        }
    }

    private NativeBuffer(Arena arena, MemorySegment segment) {
        this.arena = arena;
        this.segment = segment;
        this.cleanable = CLEANER.register(this, new Release(arena));
    }

    /** Takes ownership of an {@code AceBuffer} handle. */
    static NativeBuffer wrap(MemorySegment handle) {
        io.ace.ffi.AceBindings.Functions f = Native.fn();
        long length = f.aceBufferLen(handle);
        MemorySegment data = f.aceBufferData(handle);
        Arena arena = Arena.ofShared();
        NativeHandle.countNew();
        MemorySegment segment = data.reinterpret(length, arena, ignored -> {
            try {
                f.aceBufferFree(handle);
            } finally {
                NativeHandle.countReleased();
            }
        });
        return new NativeBuffer(arena, segment);
    }

    /** The bytes, without a copy. Valid until {@link #close()}. */
    public MemorySegment segment() {
        return segment;
    }

    /** The number of bytes. */
    public long size() {
        return segment.byteSize();
    }

    /** Copies the bytes into a new array. */
    public byte[] toByteArray() {
        return segment.toArray(ValueLayout.JAVA_BYTE);
    }

    /** Releases the native memory; later calls do nothing. */
    @Override
    public void close() {
        cleanable.clean();
    }
}
