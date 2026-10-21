package io.ace;

import io.ace.ffi.AceBindings;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.nio.charset.StandardCharsets;

/**
 * Helpers shared by the SDK classes: status checking, last-error reading and buffer handling.
 *
 * <p>The last error is thread-local in the library. Every method here reads it right after the
 * failing call on the same thread, with no blocking operation in between, so a virtual thread
 * cannot move to another carrier thread in the meantime.
 */
final class Native {
    private Native() {
        throw new AssertionError();
    }

    static AceBindings.Functions fn() {
        return NativeLibrary.functions();
    }

    /** Throws the exception for a failed call; returns normally for {@code ACE_OK}. */
    static void check(int status) {
        if (status != AceBindings.ACE_OK) {
            throw failure(status);
        }
    }

    /** The exception for a non-zero status, with the library's message and block number. */
    static AceException failure(int status) {
        AceBindings.Functions f = fn();
        String message = "ACE call failed with status " + status;
        MemorySegment pointer = f.aceLastErrorPtr();
        long length = f.aceLastErrorLen();
        if (length > 0 && pointer.address() != 0) {
            byte[] bytes = pointer.reinterpret(length).toArray(ValueLayout.JAVA_BYTE);
            message = new String(bytes, StandardCharsets.UTF_8);
        }
        return new AceException(Status.of(status), status, message, f.aceLastErrorBlock());
    }

    /** A native copy of {@code bytes} in {@code arena}. */
    static MemorySegment copyIn(byte[] bytes, Arena arena) {
        MemorySegment segment = arena.allocate(Math.max(1, bytes.length));
        segment.copyFrom(MemorySegment.ofArray(bytes));
        return segment;
    }

    /** Copies out the bytes of an {@code AceBuffer} handle and releases the handle. */
    static byte[] takeBytes(MemorySegment buffer) {
        AceBindings.Functions f = fn();
        try {
            long length = f.aceBufferLen(buffer);
            MemorySegment data = f.aceBufferData(buffer).reinterpret(length);
            return data.toArray(ValueLayout.JAVA_BYTE);
        } finally {
            f.aceBufferFree(buffer);
        }
    }

    /** The pointer stored in an out-parameter slot. */
    static MemorySegment pointerIn(MemorySegment slot) {
        return slot.get(ValueLayout.ADDRESS, 0);
    }

    /** A pointer-sized, zero-initialised out-parameter slot. */
    static MemorySegment slot(Arena arena) {
        return arena.allocate(ValueLayout.ADDRESS);
    }
}
