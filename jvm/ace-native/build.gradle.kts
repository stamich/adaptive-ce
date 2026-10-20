plugins {
    `java-library`
}

description = "Panama bindings of the ACE C ABI (generated from include/ace.h) and the Java SDK"

// The library built for this machine is packaged at native/<os>-<arch>/ inside the JAR, where
// NativeLibrary extracts it from (docs/NATIVE-INTEROP.md, "Library loading on the JVM").
val aceNativeDir: String = providers.gradleProperty("aceNativeDir")
    .orElse(rootDir.parentFile.resolve("target/release").path).get()

val platformDirectory: String = run {
    val os = System.getProperty("os.name").lowercase()
    val arch = System.getProperty("os.arch").lowercase()
    val osName = when {
        os.contains("win") -> "windows"
        os.contains("mac") || os.contains("darwin") -> "macos"
        else -> "linux"
    }
    val archName = when (arch) {
        "amd64", "x86_64" -> "x86_64"
        "aarch64", "arm64" -> "aarch64"
        else -> arch
    }
    "$osName-$archName"
}

val libraryFile: String = when {
    platformDirectory.startsWith("windows") -> "ace_ffi.dll"
    platformDirectory.startsWith("macos") -> "libace_ffi.dylib"
    else -> "libace_ffi.so"
}

tasks.jar {
    dependsOn(rootProject.tasks.named("buildNative"))
    from(file(aceNativeDir)) {
        include(libraryFile)
        into("native/$platformDirectory")
    }
}
