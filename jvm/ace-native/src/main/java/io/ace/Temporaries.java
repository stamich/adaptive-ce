package io.ace;

import java.util.ArrayDeque;
import java.util.Deque;

/**
 * Native handles that live only while one call builds its arguments, for example the dictionary
 * and schema handles that an {@code AceConfig} points to. The engine copies what it needs, so they
 * are released when the creating call returns, in reverse order of creation.
 */
final class Temporaries implements AutoCloseable {
    private final Deque<Runnable> releases = new ArrayDeque<>();

    /** Registers an action that releases one handle. */
    void onClose(Runnable release) {
        releases.push(release);
    }

    @Override
    public void close() {
        RuntimeException failure = null;
        while (!releases.isEmpty()) {
            try {
                releases.pop().run();
            } catch (RuntimeException e) {
                if (failure == null) {
                    failure = e;
                } else {
                    failure.addSuppressed(e);
                }
            }
        }
        if (failure != null) {
            throw failure;
        }
    }
}
