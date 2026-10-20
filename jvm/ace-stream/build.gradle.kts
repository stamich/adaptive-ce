plugins {
    scala
}

description = "InputStream, OutputStream and Iterator adapters over the ACE SDK (no stream-library dependency)"

dependencies {
    api(project(":ace-sdk"))
}
