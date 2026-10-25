package io.ace;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.jar.JarEntry;
import java.util.jar.JarOutputStream;
import org.junit.jupiter.api.Test;

/** The library packaged in a JAR is extracted and works from a clean directory. */
class PackagedLibraryTest {
    private static Path library() {
        String directory = System.getProperty("ace.native.path");
        Path path = Path.of(directory);
        return Files.isDirectory(path) ? path.resolve(System.mapLibraryName("ace_ffi")) : path;
    }

    private static Path jarWithLibrary(Path directory) throws IOException {
        Path jar = directory.resolve("ace-native-test.jar");
        String name = System.mapLibraryName("ace_ffi");
        try (JarOutputStream out = new JarOutputStream(Files.newOutputStream(jar))) {
            out.putNextEntry(new JarEntry("native/" + NativeLibrary.platformDirectory() + "/" + name));
            out.write(Files.readAllBytes(library()));
            out.closeEntry();
        }
        return jar;
    }

    @Test
    void platformDirectoryIsOneOfTheDocumentedOnes() {
        assertTrue(List.of("linux-x86_64", "linux-aarch64", "macos-aarch64", "macos-x86_64", "windows-x86_64")
                .contains(NativeLibrary.platformDirectory()), NativeLibrary.platformDirectory());
    }

    @Test
    void extractsTheLibraryFromAJar() throws Exception {
        Path directory = Files.createTempDirectory("ace-jar");
        Path jar = jarWithLibrary(directory);
        try (URLClassLoader loader = new URLClassLoader(new URL[] {jar.toUri().toURL()}, null)) {
            String name = System.mapLibraryName("ace_ffi");
            Path extracted = NativeLibrary.extractPackaged(loader, NativeLibrary.platformDirectory(), name);
            assertNotNull(extracted);
            assertTrue(extracted.getFileName().toString().endsWith(name.substring(name.lastIndexOf('.'))));
            assertArrayEquals(Files.readAllBytes(library()), Files.readAllBytes(extracted));
            assertNull(NativeLibrary.extractPackaged(loader, "plan9-mips", name));
            Files.deleteIfExists(extracted);
        } finally {
            Files.deleteIfExists(jar);
            Files.deleteIfExists(directory);
        }
    }

    @Test
    void aFreshJvmLoadsTheLibraryFromTheJar() throws Exception {
        Path directory = Files.createTempDirectory("ace-clean");
        Path jar = jarWithLibrary(directory);
        try {
            String java = ProcessHandle.current().info().command().orElse("java");
            List<String> command = new ArrayList<>(List.of(java, "--enable-native-access=ALL-UNNAMED",
                    "-cp", System.getProperty("java.class.path") + System.getProperty("path.separator") + jar,
                    "io.ace.PackagedLibrarySmoke"));
            ProcessBuilder builder = new ProcessBuilder(command).directory(directory.toFile()).redirectErrorStream(true);
            builder.environment().remove("ACE_NATIVE_PATH");
            builder.environment().remove("LD_LIBRARY_PATH");
            Process process = builder.start();
            String output = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
            assertEquals(0, process.waitFor(), output);
            assertTrue(output.contains("origin=packaged in the JAR"), output);
            assertTrue(output.contains("version=0.8.0"), output);
        } finally {
            Files.deleteIfExists(jar);
            Files.deleteIfExists(directory);
        }
    }
}
