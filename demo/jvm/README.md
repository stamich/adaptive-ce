# JVM demos

Two small programs on the ACE 0.8.0 SDK. They need JDK 22 or newer and a Rust toolchain (Gradle
builds `libace_ffi` for you).

```bash
gradle -p jvm :ace-demo:runJavaDemo     # Java: statistics, verified round trip, range read
gradle -p jvm :ace-demo:runScalaDemo    # Scala 3: Either results, stream view, tier metrics
```

`JavaDemo` uses `io.ace.Engine` directly. `ScalaDemo` uses `Ace.use`, `AceInputStream` and
`TierCodec`, and handles every failure as an `AceError` value. The SDK itself is described in
[`docs/NATIVE-INTEROP.md`](../../docs/NATIVE-INTEROP.md) and [`docs/FFI.md`](../../docs/FFI.md).
