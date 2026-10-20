plugins {
    scala
}

description = "Demos of the Java and Scala SDKs (docs/NATIVE-INTEROP.md)"

dependencies {
    implementation(project(":ace-native"))
    implementation(project(":ace-model"))
    implementation(project(":ace-sdk"))
    implementation(project(":ace-stream"))
    implementation(project(":ace-tier"))
}

val aceNativeDir: String = providers.gradleProperty("aceNativeDir")
    .orElse(rootDir.parentFile.resolve("target/release").path).get()

// ./gradlew -p jvm :ace-demo:runJavaDemo   (or runScalaDemo)
for ((name, main) in mapOf("runJavaDemo" to "io.ace.demo.JavaDemo", "runScalaDemo" to "io.ace.demo.ScalaDemo")) {
    tasks.register<JavaExec>(name) {
        group = "demo"
        dependsOn(rootProject.tasks.named("buildNative"))
        classpath = sourceSets["main"].runtimeClasspath
        mainClass.set(main)
        jvmArgs("--enable-native-access=ALL-UNNAMED")
        systemProperty("ace.native.path", aceNativeDir)
    }
}
