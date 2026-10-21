package io.ace;

/** Run by {@link PackagedLibraryTest} in a fresh JVM: prints where the library came from and one round trip. */
public final class PackagedLibrarySmoke {
    private PackagedLibrarySmoke() {}

    public static void main(String[] args) {
        try (Engine engine = Engine.create()) {
            byte[] input = TestData.text(50_000);
            byte[] restored = engine.decompress(engine.compress(input));
            if (!java.util.Arrays.equals(input, restored)) {
                System.out.println("MISMATCH");
                System.exit(2);
            }
        }
        System.out.println("origin=" + NativeLibrary.origin());
        System.out.println("version=" + NativeLibrary.engineVersion());
    }
}
