package io.ace;

import io.ace.ffi.AceBindings;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;

/**
 * The read and write callbacks of a streaming call, backed by Java streams.
 *
 * <p>An upcall that throws would crash the JVM, so both callbacks catch everything, remember the
 * first failure and report an error status to the library. {@link #rethrow(int)} then raises the
 * remembered failure after the native call returned.
 */
final class Callbacks {
    private static final int CHUNK = 64 * 1024;

    private final InputStream in;
    private final OutputStream out;
    private final MemorySegment readStub;
    private final MemorySegment writeStub;
    private Throwable failure;

    Callbacks(InputStream in, OutputStream out, Arena arena) {
        this.in = in;
        this.out = out;
        this.readStub = AceBindings.aceReadFnStub(this::read, arena);
        this.writeStub = AceBindings.aceWriteFnStub(this::write, arena);
    }

    MemorySegment readStub() {
        return readStub;
    }

    MemorySegment writeStub() {
        return writeStub;
    }

    private long read(MemorySegment context, MemorySegment buffer, long capacity) {
        try {
            byte[] chunk = new byte[(int) Math.min(capacity, CHUNK)];
            int count;
            do {
                count = in.read(chunk);
            } while (count == 0);
            if (count < 0) {
                return 0;
            }
            buffer.reinterpret(count).copyFrom(MemorySegment.ofArray(chunk).asSlice(0, count));
            return count;
        } catch (Throwable t) {
            remember(t);
            return -1;
        }
    }

    private int write(MemorySegment context, MemorySegment buffer, long length) {
        try {
            byte[] chunk = buffer.reinterpret(length).toArray(ValueLayout.JAVA_BYTE);
            out.write(chunk);
            return AceBindings.ACE_OK;
        } catch (Throwable t) {
            remember(t);
            return AceBindings.ACE_ERR_IO;
        }
    }

    private void remember(Throwable t) {
        if (failure == null) {
            failure = t;
        }
    }

    /** Raises the failure of a callback, if there was one, instead of the generic I/O status. */
    void rethrow(int status) throws IOException {
        if (failure == null) {
            return;
        }
        if (failure instanceof IOException io) {
            throw io;
        }
        if (failure instanceof RuntimeException runtime) {
            throw runtime;
        }
        if (failure instanceof Error error) {
            throw error;
        }
        throw new IOException("stream callback failed (status " + status + ")", failure);
    }
}
