package io.ace.sdk

import io.ace.model.*
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

import java.lang.foreign.{Arena, MemorySegment, ValueLayout}

class AceSdkTest:

  private val small = AceConfig(blockSize = 32 * 1024)

  private def ok[A](r: Either[AceError, A]): A = r match
    case Right(a) => a
    case Left(e)  => fail(s"unexpected error: $e").asInstanceOf[A]

  private def err[A](r: Either[AceError, A]): AceError = r match
    case Left(e)  => e
    case Right(_) => fail("expected an error").asInstanceOf[AceError]

  @Test def roundTripsUnderEveryProfile(): Unit =
    val inputs = List(Array.emptyByteArray, Array[Byte](42), Data.counters(80_000), Data.text(200_000), Data.random(50_000, 3))
    for profile <- Profile.values do
      ok(Ace.use(small.copy(profile = profile)) { codec =>
        inputs.foreach { in =>
          val packed = ok(codec.compress(in))
          assertArrayEquals(in, ok(codec.decompress(packed)), s"$profile ${in.length}")
        }
        Right(())
      })

  @Test def statsAreConverted(): Unit =
    val input = Data.counters(200_000)
    val result = ok(Ace.use(small)(_.compressWithStats(input)))
    assertEquals(input.length.toLong, result.stats.inputBytes)
    assertEquals(result.bytes.length.toLong, result.stats.outputBytes)
    assertTrue(result.stats.ratio > 2.0)
    assertEquals(result.stats.blocks, result.stats.planDistribution.values.sum)

  @Test def failuresBecomeValues(): Unit =
    Ace.use() { codec =>
      assertTrue(err(codec.decompress(new Array[Byte](64))).isInstanceOf[AceError.NotAceFile])
      val packed = ok(codec.compress(Data.counters(100_000)))
      val damaged = packed.clone()
      damaged(damaged.length / 2) = (damaged(damaged.length / 2) ^ 0xff).toByte
      assertTrue(err(codec.decompress(damaged)).isDataError)
      Right(())
    }

  @Test def decodeLimitsApply(): Unit =
    val packed = ok(Ace.use()(_.compress(Data.counters(100_000))))
    Ace.use(limits = DecodeLimits(maxOutputSize = 1000)) { codec =>
      assertTrue(err(codec.decompress(packed)).isInstanceOf[AceError.LimitExceeded])
      Right(())
    }

  @Test def invalidSettingsNeverReachTheNativeLibrary(): Unit =
    assertTrue(err(Ace.open(AceConfig(blockSize = -1))).isInstanceOf[AceError.InvalidArgument])
    assertTrue(err(Ace.open(limits = DecodeLimits(maxBlocks = -1))).isInstanceOf[AceError.InvalidArgument])
    assertTrue(err(Ace.open(AceConfig(blockSize = 1L << 40))).isInstanceOf[AceError.InvalidArgument])

  @Test def useClosesTheCodecEvenOnFailure(): Unit =
    var seen: AceCodec = null
    val result = Ace.use() { codec =>
      seen = codec
      codec.decompress(new Array[Byte](64))
    }
    assertTrue(result.isLeft)
    assertTrue(seen.isClosed)

  @Test def theConfigIsReadBack(): Unit =
    val c = AceConfig(profile = Profile.Dense, blockSize = 64 * 1024, accessHint = AccessHint.RandomAccess, writeIndex = false)
    Ace.use(c) { codec =>
      assertEquals(Profile.Dense, codec.config.profile)
      assertEquals(64L * 1024, codec.config.blockSize)
      assertEquals(AccessHint.RandomAccess, codec.config.accessHint)
      assertFalse(codec.config.writeIndex)
      Right(())
    }

  @Test def offHeapRoundTripAndBufferTooSmall(): Unit =
    val input = Data.text(100_000)
    val arena = Arena.ofConfined()
    try
      Ace.use(small) { codec =>
        val source = arena.allocate(input.length.toLong)
        source.copyFrom(MemorySegment.ofArray(input))
        val packed = ok(codec.compressOffHeap(source))
        try
          assertTrue(packed.size < input.length)
          val restored = ok(codec.decompressOffHeap(packed.segment))
          try assertArrayEquals(input, restored.toArray)
          finally restored.close()
          err(codec.decompressInto(packed.segment, arena.allocate(10L))) match
            case AceError.BufferTooSmall(_, required) => assertEquals(input.length.toLong, required)
            case other                                => fail(s"unexpected $other")
          val exact = arena.allocate(input.length.toLong)
          assertEquals(input.length.toLong, ok(codec.decompressInto(packed.segment, exact)))
          assertArrayEquals(input, exact.toArray(ValueLayout.JAVA_BYTE))
        finally packed.close()
        Right(())
      }
    finally arena.close()

  @Test def rangeReads(): Unit =
    val input = Data.counters(200_000)
    Ace.use(small) { codec =>
      val range = ok(codec.openRange(ok(codec.compress(input))))
      try
        assertEquals(input.length.toLong, range.length)
        assertArrayEquals(input.slice(1000, 70_000), ok(range.read(1000, 70_000)))
        assertTrue(range.lastStats.blocks >= 2)
        assertArrayEquals(input.slice(32 * 1024, 64 * 1024), ok(range.readBlock(1)))
        assertTrue(err(range.read(10, 5)).isInstanceOf[AceError.InvalidArgument])
        assertTrue(err(range.readBlock(range.blockCount)).isInstanceOf[AceError.BlockNotFound])
      finally range.close()
      Right(())
    }

  @Test def useAfterCloseIsAnError(): Unit =
    val codec = ok(Ace.open())
    codec.close()
    assertTrue(err(codec.compress(Array[Byte](1))).isInstanceOf[AceError.InvalidArgument])

  @Test def theNativeVersionIsTheProjectVersion(): Unit =
    assertEquals("0.8.0", ok(Ace.nativeVersion))
