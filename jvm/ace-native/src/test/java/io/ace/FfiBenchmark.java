package io.ace;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;

/**
 * Cost of the JVM binding on the operations of the {@code ffi} benchmark family
 * ({@code crates/ace-bench/src/families/ffi.rs}): the mixed 16 MiB workload and small buffers.
 *
 * <p>Usage: {@code FfiBenchmark <output.json> [samples]}. System properties: {@code ace.corpus.bin}
 * (the ace-corpus binary) and {@code ace.native.path}. Run it with {@code gradle -p jvm
 * :ace-native:ffiBenchmark}.
 *
 * <p>Two paths are measured for the large workload: {@code zero_copy} (off-heap input, result
 * left in the native buffer; the gated path) and {@code byte_array} (the convenience API, which
 * copies 16 MiB in and out).
 */
public final class FfiBenchmark {
    private static final int LARGE_BYTES = 16 * 1024 * 1024;
    private static final int[] SMALL_SIZES = {64, 1024, 16 * 1024};
    private static final int WARMUPS = 4;
    private static final int SMALL_CALLS_PER_SAMPLE = 400;

    private FfiBenchmark() {}

    private interface Operation {
        void run();
    }

    /** Median of the per-invocation times in ns, with the batch MAD in percent. */
    private record Timing(double medianNs, double madPercent, double minNs) {}

    private static Timing time(int samples, int invocationsPerSample, Operation operation) {
        for (int i = 0; i < WARMUPS; i++) {
            for (int j = 0; j < invocationsPerSample; j++) {
                operation.run();
            }
        }
        double[] perCall = new double[samples];
        for (int i = 0; i < samples; i++) {
            long start = System.nanoTime();
            for (int j = 0; j < invocationsPerSample; j++) {
                operation.run();
            }
            perCall[i] = (System.nanoTime() - start) / (double) invocationsPerSample;
        }
        double[] sorted = perCall.clone();
        Arrays.sort(sorted);
        double median = median(sorted);
        double[] deviations = new double[samples];
        for (int i = 0; i < samples; i++) {
            deviations[i] = Math.abs(perCall[i] - median);
        }
        Arrays.sort(deviations);
        return new Timing(median, 100.0 * median(deviations) / median, sorted[0]);
    }

    private static double median(double[] sorted) {
        int mid = sorted.length / 2;
        return sorted.length % 2 == 1 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2.0;
    }

    private static byte[] mixed(int bytes) throws Exception {
        String binary = System.getProperty("ace.corpus.bin");
        if (binary == null || !Files.isExecutable(Path.of(binary))) {
            throw new IllegalStateException("set -Dace.corpus.bin to the ace-corpus binary, got " + binary);
        }
        Path file = Files.createTempFile("ace-ffi-bench-", ".bin");
        try {
            Process process = new ProcessBuilder(binary, "mixed", Integer.toString(bytes), file.toString())
                    .redirectErrorStream(true)
                    .start();
            String output = new String(process.getInputStream().readAllBytes());
            if (process.waitFor() != 0) {
                throw new IllegalStateException(output);
            }
            return Files.readAllBytes(file);
        } finally {
            Files.deleteIfExists(file);
        }
    }

    private static String row(String path, String mode, int bytes, Timing t) {
        return String.format(Locale.ROOT,
                "{\"path\":\"%s\",\"mode\":\"%s\",\"bytes_processed\":%d,\"median_ns\":%.1f,"
                        + "\"min_ns\":%.1f,\"mad_percent\":%.3f}",
                path, mode, bytes, t.medianNs, t.minNs, t.madPercent);
    }

    public static void main(String[] args) throws Exception {
        Path output = Path.of(args[0]);
        int samples = args.length > 1 ? Integer.parseInt(args[1]) : 15;
        byte[] large = mixed(LARGE_BYTES);
        List<String> rows = new ArrayList<>();

        try (Engine engine = Engine.create(); Arena arena = Arena.ofConfined()) {
            MemorySegment source = arena.allocate(large.length);
            source.copyFrom(MemorySegment.ofArray(large));
            byte[] packed = engine.compress(large);
            MemorySegment packedSegment = arena.allocate(packed.length);
            packedSegment.copyFrom(MemorySegment.ofArray(packed));

            rows.add(row("compress", "zero_copy", large.length, time(samples, 1, () -> {
                try (NativeBuffer out = engine.compress(source)) {
                    if (out.size() == 0) {
                        throw new IllegalStateException();
                    }
                }
            })));
            rows.add(row("decompress", "zero_copy", large.length, time(samples, 1, () -> {
                try (NativeBuffer out = engine.decompress(packedSegment)) {
                    if (out.size() != large.length) {
                        throw new IllegalStateException();
                    }
                }
            })));
            rows.add(row("compress", "byte_array", large.length, time(samples, 1, () -> engine.compress(large))));
            rows.add(row("decompress", "byte_array", large.length, time(samples, 1, () -> engine.decompress(packed))));

            for (int size : SMALL_SIZES) {
                byte[] small = Arrays.copyOf(large, size);
                byte[] smallPacked = engine.compress(small);
                rows.add(row("small_compress", "byte_array_" + size, size,
                        time(samples, SMALL_CALLS_PER_SAMPLE, () -> engine.compress(small))));
                rows.add(row("small_decompress", "byte_array_" + size, size,
                        time(samples, SMALL_CALLS_PER_SAMPLE, () -> engine.decompress(smallPacked))));
            }
        }

        String json = String.format(Locale.ROOT,
                "{\"schema\":\"ace-ffi-jvm-1\",\"jdk\":\"%s\",\"os\":\"%s %s\",\"samples\":%d,\"workloads\":[%s]}%n",
                System.getProperty("java.vm.version"), System.getProperty("os.name"), System.getProperty("os.arch"),
                samples, String.join(",", rows));
        Files.writeString(output, json);
        System.out.print(json);
    }
}
