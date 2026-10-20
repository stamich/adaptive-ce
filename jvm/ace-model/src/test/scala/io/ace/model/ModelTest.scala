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
