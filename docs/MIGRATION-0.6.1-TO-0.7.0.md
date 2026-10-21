# Migrating from ACE 0.6.1 to 0.7.0

* **Rust API**: unchanged. Nothing to change in calling code.
* **Bytes**: unchanged. Hashes of compressed output from 0.6.1 stay valid.
* **Format**: unchanged (1.0 – 1.5 read, 1.3 / 1.4 / 1.5 written).
* **New: C ABI.** Link `libace_ffi` and include `include/ace.h`. Read
  [`NATIVE-INTEROP.md`](NATIVE-INTEROP.md) first (status codes, last error, handle and buffer
  ownership), then [`FFI.md`](FFI.md). The ABI version is 1 and is independent of the file format
  version; check `ace_abi_version()` before anything else.
* **New: JVM SDK.** Java 22 or newer, started with `--enable-native-access=ALL-UNNAMED`.
  Gradle modules `ace-native` (Java), `ace-model`, `ace-sdk`, `ace-stream`, `ace-tier` (Scala 3).
  Set `-Dace.native.path=<dir or file>` (or `ACE_NATIVE_PATH`) to use a library you built; a JAR
  built by `gradle -p jvm :ace-native:jar` carries the library of the build machine.
* **Removed: `integrations/`.** The CLI-based `java-demo` and `scala-demo` are gone; use the SDK
  and the demos in `demo/jvm/` (`gradle -p jvm :ace-demo:runJavaDemo`, `:ace-demo:runScalaDemo`).
* **Contributors.**
  * CI workflow `ace-ci-0.7.0` (was `ace-ci-0.6.1`): update required status-check names in the
    repository settings. New jobs: `aarch64`, `jvm` (JDK 22 and 25).
  * The `pr` job needs cbindgen 0.29.0 (`cargo install cbindgen --locked --version 0.29.0`) and a
    C compiler; it checks `include/ace.h`, the generated Java bindings
    (`tools/ace-genbindings0.7.0.py --check`) and the C smoke test.
  * `include/ace.h` and `jvm/ace-native/src/main/java/io/ace/ffi/AceBindings.java` are generated:
    regenerate with `tools/ace-check_header0.7.0.sh --write` and
    `python3 tools/ace-genbindings0.7.0.py --write`.
  * Scripts and tools carry the suffix 0.7.0; the A/B base and the stored baseline are 0.6.1.
  * Fuzz: 20 targets.
