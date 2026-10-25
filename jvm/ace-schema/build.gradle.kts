plugins {
    scala
    `java-library`
}

description = "Storage policy for ACE 0.8.0: record schemas from case classes, a dictionary store and per-tier profiles"

dependencies {
    api(project(":ace-sdk"))
    api(project(":ace-tier"))
}
