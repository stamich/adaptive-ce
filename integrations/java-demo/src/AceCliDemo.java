import java.io.IOException;
import java.util.List;

/**
 * Minimal Java demonstration of the stable ACE CLI boundary.
 *
 * <p>This is intentionally not a production binding. Native JVM integration is deferred to the
 * planned FFM/Panama milestone; ACE 0.2.1-buildfix1 keeps its core and wire format Rust-first.</p>
 */
public final class AceCliDemo {
    /** Prevents instantiation of this utility-only demonstration class. */
    private AceCliDemo() { }

    /**
     * Runs {@code ace verify} for the supplied archive path.
     *
     * @param aceExecutable path or command name of the ACE CLI executable
     * @param archive path of the ACE archive to verify
     * @return process exit status
     * @throws IOException when the process cannot be started
     * @throws InterruptedException when the current thread is interrupted while waiting
     */
    public static int verify(String aceExecutable, String archive) throws IOException, InterruptedException {
        Process process = new ProcessBuilder(List.of(aceExecutable, "verify", archive))
                .inheritIO()
                .start();
        return process.waitFor();
    }

    /**
     * Command-line entry point used by the optional Java integration example.
     *
     * @param args {@code <ace-executable> <archive.ace>}
     * @throws Exception when process execution fails
     */
    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("usage: AceCliDemo <ace-executable> <archive.ace>");
        }
        System.exit(verify(args[0], args[1]));
    }
}
