# Java CLI demonstration

This example is intentionally process-based. It documents the stable CLI boundary without introducing JNI or Panama into ACE (the 0.4 line has no native JVM ABI; see the roadmap). The class is fully documented with Javadoc.

Source: `src/main/java/io/ace/demo/AceCliDemo.java` (standard Maven/Gradle layout).

```bash
javac -d out src/main/java/io/ace/demo/AceCliDemo.java
java -cp out io.ace.demo.AceCliDemo ../../target/release/ace input.bin output.ace
```
