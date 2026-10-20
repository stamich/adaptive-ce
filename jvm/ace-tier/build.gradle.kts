plugins {
    scala
}

description = "Compression profile per storage tier (hot, warm, cold) and neutral metrics for tier-economics tools"

dependencies {
    api(project(":ace-sdk"))
}
