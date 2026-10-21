package io.ace;

/** Counters for tests and monitoring. */
public final class Diagnostics {
    private Diagnostics() {
        throw new AssertionError();
    }

    /**
     * The number of native objects (engines, readers, buffers) that were created and not yet
     * released. A leak-free program that closed everything reads 0 once the garbage collector has
     * run the safety nets.
     */
    public static long liveNativeObjects() {
        return NativeHandle.live();
    }
}
