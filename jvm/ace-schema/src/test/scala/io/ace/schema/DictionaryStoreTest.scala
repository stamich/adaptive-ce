package io.ace.schema

import io.ace.model.*
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

import java.nio.file.{Files, Path}

class DictionaryStoreTest:

  private def ok[A](r: Either[AceError, A]): A = r match
    case Right(a) => a
    case Left(e)  => fail(s"unexpected error: $e").asInstanceOf[A]

  private val first = AceDictionary(0x10L, "temperature_reading celsius".getBytes)
  private val second = AceDictionary(-1L, "release-2.7.1".getBytes) // id above 2^63, sorts last

  private def behaves(store: DictionaryStore): Unit =
    assertEquals(Seq.empty, ok(store.all))
    assertEquals(None, ok(store.get(0x10L)))
    ok(store.put(second))
    ok(store.put(first))
    ok(store.put(first)) // idempotent
    assertEquals(Seq(first, second), ok(store.all), "ordered by unsigned id")
    assertEquals(Some(first), ok(store.get(0x10L)))
    assertEquals(Some(second), ok(store.get(-1L)))
    assertTrue(ok(store.remove(0x10L)))
    assertFalse(ok(store.remove(0x10L)))
    assertEquals(Seq(second), ok(store.all))

  @Test def inMemoryStoreKeepsDictionariesByIdInUnsignedOrder(): Unit =
    behaves(DictionaryStore.inMemory)

  @Test def directoryStoreKeepsDictionariesByIdInUnsignedOrder(): Unit =
    val directory = Files.createTempDirectory("ace-dictionaries-")
    behaves(DictionaryStore.directory(directory.resolve("nested")))

  @Test def aDirectoryStoreSurvivesBeingReopened(): Unit =
    val directory = Files.createTempDirectory("ace-dictionaries-")
    ok(DictionaryStore.directory(directory).put(first))
    assertEquals(Seq(first), ok(DictionaryStore.directory(directory).all))

  @Test def invalidDictionariesAreRejectedByBothStores(): Unit =
    val bad = AceDictionary(0L, Array[Byte](1))
    val directory = Files.createTempDirectory("ace-dictionaries-")
    for store <- List(DictionaryStore.inMemory, DictionaryStore.directory(directory)) do
      assertTrue(store.put(bad).left.exists(_.isInstanceOf[AceError.InvalidArgument]))

  @Test def aDamagedFileIsReportedNotReturned(): Unit =
    val directory = Files.createTempDirectory("ace-dictionaries-")
    val store = DictionaryStore.directory(directory)
    ok(store.put(first))
    val file: Path = directory.resolve(f"${first.id}%016x.acedict")
    val raw = Files.readAllBytes(file)
    raw(raw.length - 1) = (raw(raw.length - 1) ^ 0x55).toByte
    Files.write(file, raw)
    assertTrue(store.get(first.id).left.exists(_.isInstanceOf[AceError.ChecksumMismatch]))
    assertTrue(store.all.isLeft)
    Files.write(file, Array[Byte](1, 2, 3))
    assertTrue(store.get(first.id).left.exists(_.isInstanceOf[AceError.Corrupt]))

  @Test def noTemporaryFilesAreLeftBehind(): Unit =
    val directory = Files.createTempDirectory("ace-dictionaries-")
    ok(DictionaryStore.directory(directory).put(first))
    val names = Files.list(directory).toArray.map(_.toString)
    assertEquals(1, names.length)
    assertTrue(names.head.endsWith(".acedict"))
