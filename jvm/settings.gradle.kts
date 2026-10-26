// ACE JVM SDK (docs/NATIVE-INTEROP.md). Java in ace-native (Panama bindings and the Java SDK);
// Scala 3 in ace-model (types), ace-sdk (Either-based API) and ace-stream (stream adapters), ace-tier (tier policy and metrics),
// ace-schema (schemas from case classes, dictionary store, storage profiles per tier).
rootProject.name = "ace-jvm"

include("ace-native", "ace-model", "ace-sdk", "ace-stream", "ace-tier", "ace-schema")

include("ace-demo")
project(":ace-demo").projectDir = file("../demo/jvm")
