plugins {
    scala
    `java-library`
}

description = "Compression profile per storage tier (hot, warm, cold) and neutral metrics for tier-economics tools"

dependencies {
    api(project(":ace-sdk"))
}
