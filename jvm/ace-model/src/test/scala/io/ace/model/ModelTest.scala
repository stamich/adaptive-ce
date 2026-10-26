package io.ace.model

import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

class ModelTest:

  @Test def defaultsAreValid(): Unit =
    assertEquals(Nil, AceConfig.Default.validate)
    assertEquals(Nil, DecodeLimits.Default.validate)
    assertEquals(Profile.Balanced, AceConfig.Default.profile)
    assertTrue(AceConfig.Default.writeIndex)

  @Test def negativeSettingsAreReported(): Unit =
    val problems = AceConfig(blockSize = -1, threads = -2).validate
    assertEquals(2, problems.size)
    assertTrue(problems.exists(_.contains("blockSize")))
    assertEquals(List("maxBlocks must not be negative"), DecodeLimits(maxBlocks = -1).validate)

  @Test def statisticsAreDerived(): Unit =
    val stats = CompressionStats(1000, 250, 5, 1, 0, 0, 1, 0, 0, 4.0, Map("lz+huffman" -> 1L))
    assertEquals(0.75, stats.savings, 1e-12)
    assertEquals(0.0, stats.copy(inputBytes = 0).savings, 1e-12)
    assertEquals(2.0, RangeReadStats(100, 200, 2).readAmplification, 1e-12)
    assertEquals(0.0, RangeReadStats(0, 200, 2).readAmplification, 1e-12)

  @Test def errorsClassifyThemselves(): Unit =
    assertTrue(AceError.Corrupt("x").isDataError)
    assertTrue(AceError.ChecksumMismatch("x", Some(3)).isDataError)
    assertFalse(AceError.InvalidArgument("x").isDataError)
    assertFalse(AceError.Io("x").isPermanent)
    assertFalse(AceError.Internal("x").isPermanent)
    assertTrue(AceError.LimitExceeded("x").isPermanent)
    assertEquals("boom", AceError.Unknown("boom", 99).message)

  @Test def errorsMatchExhaustively(): Unit =
    def describe(e: AceError): String = e match
      case AceError.InvalidArgument(_)     => "argument"
      case AceError.NotAceFile(_)          => "not-ace"
      case AceError.Unsupported(_)         => "unsupported"
      case AceError.Corrupt(_)             => "corrupt"
      case AceError.ChecksumMismatch(_, b) => s"checksum ${b.getOrElse(-1L)}"
      case AceError.LimitExceeded(_)       => "limit"
      case AceError.BlockNotFound(_, b)    => s"block ${b.getOrElse(-1L)}"
      case AceError.Io(_)                  => "io"
      case AceError.BufferTooSmall(_, n)   => s"small $n"
      case AceError.Internal(_)            => "internal"
      case AceError.Unknown(_, c)          => s"unknown $c"
      case AceError.Unavailable(_)         => "unavailable"
    assertEquals("checksum 7", describe(AceError.ChecksumMismatch("m", Some(7))))
    assertEquals("small 10", describe(AceError.BufferTooSmall("m", 10)))

  @Test def storageOptionsAreOffByDefault(): Unit =
    val config = AceConfig.Default
    assertEquals(None, config.schema)
    assertEquals(None, config.dictionary)
    assertEquals(None, config.segmentation)

  @Test def schemasAreValidated(): Unit =
    assertEquals(Nil, Schema.of(Column.unsigned(8), Column.float(8), Column.bytes(5)).validate)
    assertEquals(21, Schema.of(Column.unsigned(8), Column.float(8), Column.bytes(5)).recordSize)
    assertEquals(1, Schema(Nil).validate.size)
    assertEquals(1, Schema.of(Column.unsigned(3)).validate.size)
    assertEquals(1, Schema.of(Column.float(2)).validate.size)
    assertEquals(1, Schema.of(Column.bytes(0)).validate.size)

  @Test def dictionariesCompareByContent(): Unit =
    val a = AceDictionary(7L, Array[Byte](1, 2, 3))
    assertEquals(a, AceDictionary(7L, Array[Byte](1, 2, 3)))
    assertEquals(a.hashCode, AceDictionary(7L, Array[Byte](1, 2, 3)).hashCode)
    assertNotEquals(a, AceDictionary(8L, Array[Byte](1, 2, 3)))
    assertNotEquals(a, AceDictionary(7L, Array[Byte](1, 2, 4)))
    assertEquals(Nil, a.validate)
    assertEquals(1, AceDictionary(0L, Array[Byte](1)).validate.size)
    assertEquals(1, AceDictionary(1L, Array.emptyByteArray).validate.size)
    assertEquals(1, AceDictionary(1L, new Array[Byte](AceDictionary.MaxSize + 1)).validate.size)

  @Test def configValidationIncludesTheStorageOptions(): Unit =
    val config = AceConfig(
      schema = Some(Schema(Nil)),
      dictionary = Some(DictionaryUse(AceDictionary(0L, Array[Byte](1)))),
      segmentation = Some(Segmentation(maxSegments = -1))
    )
    assertEquals(3, config.validate.size)
