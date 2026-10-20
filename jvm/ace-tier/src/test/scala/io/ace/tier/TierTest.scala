package io.ace.tier

import io.ace.model.*
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

import java.nio.{ByteBuffer, ByteOrder}

class TierTest:

  private def doubles(count: Int): Array[Byte] =
    val b = ByteBuffer.allocate(count * 8).order(ByteOrder.LITTLE_ENDIAN)
    (0 until count).foreach(i => b.putDouble(math.round(math.sin(i / 50.0) * 10000.0) / 100.0))
    b.array()

  private def ok[A](r: Either[AceError, A]): A = r match
    case Right(a) => a
    case Left(e)  => fail(s"unexpected error: $e").asInstanceOf[A]

  @Test def theDefaultPolicyTradesSizeForSpeedOnHotData(): Unit =
    val p = TierPolicy.Default
    assertEquals(Profile.Fast, p.configFor(Tier.Hot).profile)
    assertEquals(Profile.Balanced, p.configFor(Tier.Warm).profile)
    assertEquals(Profile.Dense, p.configFor(Tier.Cold).profile)
    assertTrue(p.hot.blockSize < p.warm.blockSize && p.warm.blockSize < p.cold.blockSize)
    assertEquals(Nil, Tier.values.toList.flatMap(t => p.configFor(t).validate))

  @Test def aTierCanBeReplaced(): Unit =
    val custom = TierPolicy.Default.withTier(Tier.Cold, AceConfig(profile = Profile.Balanced))
    assertEquals(Profile.Balanced, custom.configFor(Tier.Cold).profile)
    assertEquals(Profile.Fast, custom.configFor(Tier.Hot).profile)

  @Test def metricsComeFromTheStatistics(): Unit =
    val stats = CompressionStats(1000, 250, 5, 2, 0, 0, 2, 0, 0, 4.0, Map("lz+huffman" -> 2L))
    val m = TierMetrics.fromStats(Tier.Warm, stats, encodeNanos = 5000, decodeNanos = 2000)
    assertEquals(5.0, m.encodeNsPerByte, 1e-12)
    assertEquals(2.0, m.decodeNsPerByte, 1e-12)
    assertEquals(0.25, m.storedFraction, 1e-12)
    assertEquals(2000.0, m.decodeCostNs(1000), 1e-9)
    val empty = TierMetrics.fromStats(Tier.Hot, stats.copy(inputBytes = 0, outputBytes = 0, ratio = 1.0), 10, 10)
    assertEquals(0.0, empty.encodeNsPerByte, 1e-12)
    assertEquals(1.0, empty.storedFraction, 1e-12)

  @Test def everyTierRoundTripsAndReportsMetrics(): Unit =
    val sample = doubles(120_000)
    val codec = ok(TierCodec.open())
    try
      val all = ok(codec.measureAll(sample))
      assertEquals(Tier.values.toList, all.map(_.tier))
      all.foreach { m =>
        assertEquals(sample.length.toLong, m.inputBytes)
        assertTrue(m.ratio > 1.0, m.toString)
        assertTrue(m.encodeNsPerByte > 0.0 && m.decodeNsPerByte > 0.0, m.toString)
      }
      val byTier = all.map(m => m.tier -> m).toMap
      assertTrue(byTier(Tier.Cold).outputBytes <= byTier(Tier.Hot).outputBytes)
      val packed = ok(codec.compress(Tier.Cold, sample))
      assertArrayEquals(sample, ok(codec.decompress(packed.bytes)))
    finally codec.close()

  @Test def anInvalidPolicyFailsToOpen(): Unit =
    val bad = TierPolicy.Default.withTier(Tier.Hot, AceConfig(blockSize = -1))
    assertTrue(TierCodec.open(bad).left.exists(_.isInstanceOf[AceError.InvalidArgument]))
