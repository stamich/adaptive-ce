plugins {
    scala
}

description = "Scala 3 SDK over the ACE native library: Either-based API, scopes and off-heap buffers"

dependencies {
    api(project(":ace-model"))
    api(project(":ace-native"))
}
