package io.ace;

import io.ace.ffi.AceBindings;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.io.IOException;
import java.io.InputStream;
import java.lang.foreign.ValueLayout;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * Locates and binds the {@code ace_ffi} native library, once per JVM.
 *
 * <p>The library is searched in this order, and the first one that loads wins:
 *
 * <ol>
 *   <li>the system property {@code ace.native.path} (a library file or a directory that contains
 *       it);
 *   <li>the environment variable {@code ACE_NATIVE_PATH} (same meaning);
 *   <li>the platform library path ({@code LD_LIBRARY_PATH}, {@code java.library.path}).
 * </ol>
 *
 * <p>After loading, the ABI version of the library is compared with the one the bindings were
 * generated for; a mismatch is an {@link AceBindings.AbiMismatchException}.
 */
public final class NativeLibrary {
    /** System property naming the library file or its directory. */
    public static final String PATH_PROPERTY = "ace.native.path";

    /** Environment variable naming the library file or its directory. */
    public static final String PATH_ENVIRONMENT = "ACE_NATIVE_PATH";

    private static final String LIBRARY_NAME = "ace_ffi";

    private static volatile Loaded loaded;

    private NativeLibrary() {
        throw new AssertionError();
    }

    private record Loaded(AceBindings.Functions functions, String origin) {}

    /** The bound functions of the native library. */
    static AceBindings.Functions functions() {
        return load().functions();
    }

    /** Where the library was loaded from: a file path or {@code "library path"}. */
    public static String origin() {
        return load().origin();
    }

    /** The ABI version reported by the loaded library. */
    public static int abiVersion() {
        return functions().aceAbiVersion();
    }

    /** The crate version reported by the loaded library, for diagnostics. */
    public static String engineVersion() {
        MemorySegment text = functions().aceEngineVersion();
        MemorySegment bytes = text.reinterpret(256);
        int length = 0;
        while (length < 256 && bytes.get(ValueLayout.JAVA_BYTE, length) != 0) {
            length++;
        }
        return new String(bytes.asSlice(0, length).toArray(ValueLayout.JAVA_BYTE), StandardCharsets.UTF_8);
    }

    /**
     * The platform directory under {@code /native/} in the JAR: {@code linux-x86_64},
     * {@code linux-aarch64}, {@code macos-aarch64} or {@code windows-x86_64}.
     */
    static String platformDirectory() {
        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        String arch = System.getProperty("os.arch", "").toLowerCase(Locale.ROOT);
        String osName = os.contains("win") ? "windows" : os.contains("mac") || os.contains("darwin") ? "macos" : "linux";
        String archName = arch.equals("amd64") || arch.equals("x86_64") ? "x86_64"
                : arch.equals("aarch64") || arch.equals("arm64") ? "aarch64" : arch;
        return osName + "-" + archName;
    }

    /**
     * Copies the library packaged at {@code /native/<platform>/<file>} to a temporary file that is
     * deleted on exit.
     *
     * @return the file, or {@code null} when the class loader has no such resource
     * @throws IOException when the copy fails
     */
    static Path extractPackaged(ClassLoader loader, String platform, String fileName) throws IOException {
        String resource = "native/" + platform + "/" + fileName;
        try (InputStream in = loader == null ? null : loader.getResourceAsStream(resource)) {
            if (in == null) {
                return null;
            }
            int dot = fileName.lastIndexOf('.');
            String suffix = dot < 0 ? "" : fileName.substring(dot);
            String prefix = dot < 0 ? fileName : fileName.substring(0, dot);
            Path file = Files.createTempFile(prefix + "-", suffix);
            file.toFile().deleteOnExit();
            Files.copy(in, file, StandardCopyOption.REPLACE_EXISTING);
            return file;
        }
    }

    private static Loaded load() {
        Loaded current = loaded;
        if (current == null) {
            synchronized (NativeLibrary.class) {
                current = loaded;
                if (current == null) {
                    current = locateAndBind();
                    loaded = current;
                }
            }
        }
        return current;
    }

    private static Loaded locateAndBind() {
        List<String> attempts = new ArrayList<>();
        String fileName = System.mapLibraryName(LIBRARY_NAME);

        for (String candidate : new String[] {System.getProperty(PATH_PROPERTY), System.getenv(PATH_ENVIRONMENT)}) {
            if (candidate == null || candidate.isBlank()) {
                continue;
            }
            Path path = Path.of(candidate);
            Path file = Files.isDirectory(path) ? path.resolve(fileName) : path;
            if (!Files.isRegularFile(file)) {
                attempts.add(file + " (not a file)");
                continue;
            }
            return new Loaded(AceBindings.bind(SymbolLookup.libraryLookup(file, Arena.global())), file.toString());
        }

        try {
            Path extracted = extractPackaged(NativeLibrary.class.getClassLoader(), platformDirectory(), fileName);
            if (extracted != null) {
                return new Loaded(AceBindings.bind(SymbolLookup.libraryLookup(extracted, Arena.global())),
                        "packaged in the JAR, extracted to " + extracted);
            }
            attempts.add("/native/" + platformDirectory() + "/" + fileName + " in the JAR (not packaged)");
        } catch (IOException e) {
            attempts.add("the packaged library could not be extracted (" + e.getMessage() + ")");
        }

        try {
            SymbolLookup lookup = SymbolLookup.libraryLookup(fileName, Arena.global());
            return new Loaded(AceBindings.bind(lookup), "library path");
        } catch (IllegalArgumentException e) {
            attempts.add(fileName + " on the library path (" + e.getMessage() + ")");
        }
        throw new UnsatisfiedLinkError("cannot load the ACE native library; tried " + String.join(", ", attempts)
                + ". Set -D" + PATH_PROPERTY + " or " + PATH_ENVIRONMENT + " to the library or its directory.");
    }
}
