package io.ace.demo;

import java.io.IOException;
import java.nio.file.Path;

/**
 * Minimal Java integration example for ACE 0.2.
 *
 * <p>The milestone deliberately does not expose a native JVM ABI yet. This class demonstrates the
 * process boundary only, keeping Java outside the compression core until the planned FFM integration.
 */
public final class AceCliDemo {
    /** Prevents instantiation of this command-only utility class. */
    private AceCliDemo() {}

    /**
     * Compresses one file by invoking the ACE command-line interface.
     *
     * @param aceExecutable path to the built {@code ace} executable
     * @param input source file
     * @param output destination ACE file
     * @return process exit code
     * @throws IOException when the process cannot be started
     * @throws InterruptedException when the calling thread is interrupted while waiting
     */
    public static int compress(Path aceExecutable, Path input, Path output)
            throws IOException, InterruptedException {
        Process process = new ProcessBuilder(
                aceExecutable.toString(), "compress", input.toString(), output.toString())
                .inheritIO()
                .start();
        return process.waitFor();
    }

    /**
     * Runs a small command-line demonstration when three paths are provided.
     *
     * @param args executable, input and output paths in that order
     * @throws Exception when process creation or waiting fails
     */
    public static void main(String[] args) throws Exception {
        if (args.length != 3) {
            throw new IllegalArgumentException("usage: AceCliDemo ACE_EXECUTABLE INPUT OUTPUT");
        }
        int code = compress(Path.of(args[0]), Path.of(args[1]), Path.of(args[2]));
        if (code != 0) {
            throw new IllegalStateException("ACE process failed with exit code " + code);
        }
    }
}
