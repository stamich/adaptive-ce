package io.ace;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import org.junit.jupiter.api.Test;

/** Misuse: double close, use after close, forgotten close, concurrent close. No native leak. */
class LifetimeTest {
    @Test
    void closeIsIdempotent() {
        long before = Diagnostics.liveNativeObjects();
        Engine engine = Engine.create();
        RangeReader reader = engine.openRange(engine.compress(TestData.text(10_000)));
        NativeBuffer buffer = engine.compress(MemorySegment.NULL);
        assertFalse(engine.isClosed());
        engine.close();
        engine.close();
        reader.close();
        reader.close();
        buffer.close();
        buffer.close();
        assertTrue(engine.isClosed());
        assertTrue(reader.isClosed());
        assertEquals(before, Diagnostics.liveNativeObjects());
    }

    @Test
    void useAfterCloseFailsCleanly() {
        long before = Diagnostics.liveNativeObjects();
        Engine engine = Engine.create();
        byte[] packed = engine.compress(TestData.text(10_000));
        RangeReader reader = engine.openRange(packed);
        NativeBuffer buffer = engine.compress(MemorySegment.NULL);
        MemorySegment segment = buffer.segment();
        engine.close();
        reader.close();
        buffer.close();

        assertThrows(IllegalStateException.class, () -> engine.compress(new byte[1]));
        assertThrows(IllegalStateException.class, () -> engine.decompress(packed));
        assertThrows(IllegalStateException.class, () -> engine.openRange(packed));
        assertThrows(IllegalStateException.class, () -> reader.read(0, 1));
        assertThrows(IllegalStateException.class, () -> reader.readBlock(0));
        assertThrows(IllegalStateException.class, reader::lastStats);
        assertThrows(IllegalStateException.class, () -> segment.get(ValueLayout.JAVA_BYTE, 0));
        assertEquals(before, Diagnostics.liveNativeObjects());
    }

    @Test
    void nullArgumentsAreRejectedBeforeTheNativeCall() {
        try (Engine engine = Engine.create()) {
            assertThrows(NullPointerException.class, () -> engine.compress((byte[]) null));
            assertThrows(NullPointerException.class, () -> engine.decompress((byte[]) null));
            assertThrows(NullPointerException.class, () -> engine.compress((MemorySegment) null));
            assertThrows(NullPointerException.class, () -> engine.openRange((byte[]) null));
            assertThrows(NullPointerException.class, () -> engine.compressStream(1, null, null));
        }
        assertThrows(NullPointerException.class, () -> Engine.create(null));
    }

    @Test
    void failedCallsLeakNothing() {
        long before = Diagnostics.liveNativeObjects();
        try (Engine engine = Engine.create()) {
            for (int i = 0; i < 50; i++) {
                assertThrows(AceException.class, () -> engine.decompress(new byte[16]));
                assertThrows(AceException.class, () -> engine.openRange(new byte[16]));
            }
        }
        assertEquals(before, Diagnostics.liveNativeObjects());
    }

    @Test
    void aForgottenEngineIsReleasedByTheGarbageCollector() throws InterruptedException {
        long before = Diagnostics.liveNativeObjects();
        forget();
        long deadline = System.nanoTime() + 20_000_000_000L;
        while (Diagnostics.liveNativeObjects() > before && System.nanoTime() < deadline) {
            System.gc();
            Thread.sleep(50);
        }
        assertEquals(before, Diagnostics.liveNativeObjects());
    }

    private static void forget() {
        Engine engine = Engine.create();
        engine.compress(TestData.text(1000));
        NativeBuffer buffer = engine.compress(MemorySegment.NULL);
        assertTrue(buffer.size() > 0);
    }

    @Test
    void anEngineIsSafeToShareBetweenThreads() throws Exception {
        byte[] input = TestData.counters(60_000);
        long before = Diagnostics.liveNativeObjects();
        try (Engine engine = Engine.create(TestData.smallBlocks())) {
            byte[] expected = engine.compress(input);
            ExecutorService pool = Executors.newFixedThreadPool(6);
            try {
                List<Future<byte[]>> results = new ArrayList<>();
                for (int i = 0; i < 24; i++) {
                    results.add(pool.submit(() -> engine.decompress(engine.compress(input))));
                }
                for (Future<byte[]> result : results) {
                    assertArrayEquals(input, result.get());
                }
                assertArrayEquals(expected, engine.compress(input));
            } finally {
                pool.shutdownNow();
            }
        }
        assertEquals(before, Diagnostics.liveNativeObjects());
    }

    @Test
    void closingWaitsForCallsInFlight() throws Exception {
        byte[] input = TestData.counters(400_000);
        Engine engine = Engine.create(TestData.smallBlocks());
        ExecutorService pool = Executors.newSingleThreadExecutor();
        try {
            Future<byte[]> running = pool.submit(() -> engine.compress(input));
            engine.close();
            // Either the call finished before the close, or it was refused; it never crashes.
            try {
                assertTrue(running.get().length > 0);
            } catch (java.util.concurrent.ExecutionException e) {
                assertTrue(e.getCause() instanceof IllegalStateException, String.valueOf(e.getCause()));
            }
            assertDoesNotThrow(engine::close);
        } finally {
            pool.shutdownNow();
        }
    }

    @Test
    void errorMessagesAreThreadLocal() throws Exception {
        try (Engine engine = Engine.create()) {
            ExecutorService pool = Executors.newFixedThreadPool(4);
            try {
                List<Future<Status>> results = new ArrayList<>();
                for (int i = 0; i < 40; i++) {
                    results.add(pool.submit(() -> {
                        try {
                            engine.decompress(new byte[64]);
                            return Status.OK;
                        } catch (AceException e) {
                            assertFalse(e.getMessage().isEmpty());
                            return e.status();
                        }
                    }));
                }
                for (Future<Status> result : results) {
                    assertEquals(Status.NOT_ACE_FILE, result.get());
                }
            } finally {
                pool.shutdownNow();
            }
        }
    }
}
