package io.ace;

import java.lang.foreign.MemorySegment;
import java.lang.ref.Cleaner;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.locks.ReentrantReadWriteLock;
import java.util.function.Consumer;

/**
 * Owns one native pointer: released by {@link #close()}, or by a {@link Cleaner} when the owner is
 * garbage collected without being closed (a safety net, not the intended path).
 *
 * <p>Users of the pointer bracket their native calls with {@link #enter()} and {@link #exit()};
 * {@link #close()} waits for them, so a concurrent close can never free a pointer that a call is
 * still using.
 */
final class NativeHandle {
    private static final Cleaner CLEANER = Cleaner.create();
    private static final AtomicLong LIVE = new AtomicLong();

    private final String kind;
    private final State state;
    private final Cleaner.Cleanable cleanable;
    private final ReentrantReadWriteLock lock = new ReentrantReadWriteLock();
    private boolean closed;

    /** The part the cleaner runs; it must not reference the owner. */
    private static final class State implements Runnable {
        private final MemorySegment pointer;
        private final Consumer<MemorySegment> free;
        private final AtomicBoolean released = new AtomicBoolean();

        State(MemorySegment pointer, Consumer<MemorySegment> free) {
            this.pointer = pointer;
            this.free = free;
        }

        @Override
        public void run() {
            if (released.compareAndSet(false, true)) {
                try {
                    free.accept(pointer);
                } finally {
                    LIVE.decrementAndGet();
                }
            }
        }
    }

    /**
     * @param owner the object whose reachability controls the safety net
     * @param kind a noun for error messages ("engine")
     * @param pointer the native handle
     * @param free releases the pointer; must not capture {@code owner}
     */
    NativeHandle(Object owner, String kind, MemorySegment pointer, Consumer<MemorySegment> free) {
        this.kind = kind;
        LIVE.incrementAndGet();
        this.state = new State(pointer, free);
        this.cleanable = CLEANER.register(owner, state);
    }

    static long live() {
        return LIVE.get();
    }

    /** Counts a native object that is released by other means (a {@link NativeBuffer}). */
    static void countNew() {
        LIVE.incrementAndGet();
    }

    /** Counterpart of {@link #countNew()}. */
    static void countReleased() {
        LIVE.decrementAndGet();
    }

    /** Starts a native call: returns the pointer, or fails when the handle was closed. */
    MemorySegment enter() {
        lock.readLock().lock();
        if (closed) {
            lock.readLock().unlock();
            throw new IllegalStateException("the " + kind + " is closed");
        }
        return state.pointer;
    }

    /** Ends a native call started by {@link #enter()}. */
    void exit() {
        lock.readLock().unlock();
    }

    boolean isClosed() {
        lock.readLock().lock();
        try {
            return closed;
        } finally {
            lock.readLock().unlock();
        }
    }

    /** Releases the pointer once; later calls do nothing. */
    void close() {
        lock.writeLock().lock();
        try {
            if (!closed) {
                closed = true;
                cleanable.clean();
            }
        } finally {
            lock.writeLock().unlock();
        }
    }
}
