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

// gradle -p jvm :ace-native:ffiBenchmark [-PffiOut=<file>]: the JVM side of the `ffi` benchmark
// family (see tools/ace-check_regressions0.8.0.py, section ffi).
tasks.register<JavaExec>("ffiBenchmark") {
    group = "verification"
    description = "Measures the JVM binding on the ffi benchmark workloads and writes JSON."
    dependsOn(rootProject.tasks.named("buildNative"), rootProject.tasks.named("buildCorpus"), "testClasses")
    classpath = sourceSets["test"].runtimeClasspath
    mainClass.set("io.ace.FfiBenchmark")
    jvmArgs("--enable-native-access=ALL-UNNAMED")
    systemProperty("ace.native.path", aceNativeDir)
    systemProperty("ace.corpus.bin", if (System.getProperty("os.name").lowercase().contains("win"))
        rootDir.parentFile.resolve("target/release/ace-corpus.exe").path
    else
        rootDir.parentFile.resolve("target/release/ace-corpus").path)
    args(providers.gradleProperty("ffiOut").orElse(layout.buildDirectory.file("ffi-jvm.json").get().asFile.path).get())
}
