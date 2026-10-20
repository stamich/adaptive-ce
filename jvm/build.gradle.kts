// Common configuration of the ACE JVM modules.
//
//   cargo build --release -p ace-ffi          # the native library (the `buildNative` task does it)
//   gradle -p jvm build                       # compile and test every module (JDK 22 or newer)
//
// Tests find the library through the system property `ace.native.path`, set below to the
// directory of the Cargo build; override it with -PaceNativeDir=<dir>.

val aceVersion: String by project
val aceJavaRelease: String by project
val junitVersion: String by project
val scalaVersion: String by project

val aceNativeDir: String = providers.gradleProperty("aceNativeDir")
    .orElse(rootDir.parentFile.resolve("target/release").path).get()

val buildNative = tasks.register<Exec>("buildNative") {
    group = "build"
    description = "Builds libace_ffi with Cargo (release profile)."
    workingDir = rootDir.parentFile
    commandLine("cargo", "build", "--release", "-p", "ace-ffi")
    // Skipped when the caller supplies a library with -PaceNativeDir or -PskipNativeBuild.
    onlyIf { !project.hasProperty("skipNativeBuild") && !project.hasProperty("aceNativeDir") }
}

subprojects {
    group = "io.ace"
    version = aceVersion

    repositories {
        mavenCentral()
    }

    plugins.withType<JavaPlugin> {
        tasks.withType<JavaCompile>().configureEach {
            options.release.set(aceJavaRelease.toInt())
            options.encoding = "UTF-8"
            options.compilerArgs.add("-Xlint:all")
        }

        dependencies {
            "testImplementation"(platform("org.junit:junit-bom:$junitVersion"))
            "testImplementation"("org.junit.jupiter:junit-jupiter")
            "testRuntimeOnly"("org.junit.platform:junit-platform-launcher")
        }

        plugins.withType<ScalaPlugin> {
            dependencies {
                "implementation"("org.scala-lang:scala3-library_3:$scalaVersion")
            }
            tasks.withType<ScalaCompile>().configureEach {
                scalaCompileOptions.additionalParameters = listOf("-deprecation", "-feature", "-unchecked")
            }
        }

        tasks.withType<Test>().configureEach {
            useJUnitPlatform()
            dependsOn(buildNative)
            // Panama needs native access; the property tells the loader where the library is.
            jvmArgs("--enable-native-access=ALL-UNNAMED")
            systemProperty("ace.native.path", aceNativeDir)
            systemProperty("ace.repo.root", rootDir.parentFile.path)
            testLogging {
                events("failed", "skipped")
                exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
            }
        }
    }
}
