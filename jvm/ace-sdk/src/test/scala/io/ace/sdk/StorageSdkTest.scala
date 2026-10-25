package io.ace.sdk

import io.ace.model.*
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

import java.nio.charset.StandardCharsets
import java.nio.{ByteBuffer, ByteOrder}

/** Format 1.6 through the Scala SDK: dictionaries, schemas and segmentation. */
class StorageSdkTest:

  private def ok[A](r: Either[AceError, A]): A = r match
    case Right(a) => a
    case Left(e)  => fail(s"unexpected error: $e").asInstanceOf[A]

  private def err[A](r: Either[AceError, A]): AceError = r match
    case Left(e)  => e
    case Right(_) => fail("expected an error").asInstanceOf[AceError]

  private def record(i: Int): Array[Byte] =
    s"""{"sensor":"temperature_reading","unit":"celsius","site":"plant-${i % 7}","value":${1000 + i * 13 % 97}}
""".getBytes(StandardCharsets.UTF_8)

  private val samples: Seq[Array[Byte]] = (0 until 300).map(record)
  private val input: Array[Byte] = (0 until 600).flatMap(i => record(i).toSeq).toArray

  private def table(rows: Int): Array[Byte] =
    val b = ByteBuffer.allocate(rows * 24).order(ByteOrder.LITTLE_ENDIAN)
    var i = 0L
    while i < rows do
      b.putLong(1_700_000_000L + i * 5).putDouble(100 + (i % 50) * 0.25).putLong(i % 3)
      i += 1
    b.array()

  private val small = AceConfig(blockSize = 4096)

  @Test def trainingIsDeterministicAndDictionariesAreDerivedFromContent(): Unit =
    val first = ok(Ace.trainDictionary(samples, 2048))
    assertEquals(first, ok(Ace.trainDictionary(samples, 2048)))
    assertEquals(first, ok(Ace.dictionary(first.bytes)))
    assertTrue(first.id != 0L && first.size > 0 && first.size <= 2048)

  @Test def invalidDictionariesAreReported(): Unit =
    assertTrue(err(Ace.dictionary(Array.emptyByteArray)).isInstanceOf[AceError.InvalidArgument])
    assertTrue(err(Ace.trainDictionary(samples.take(1), 1024)).isInstanceOf[AceError.InvalidArgument])
    val bad = AceConfig(dictionary = Some(DictionaryUse(AceDictionary(0L, Array[Byte](1)))))
    assertTrue(err(Ace.open(bad)).isInstanceOf[AceError.InvalidArgument])

  @Test def embeddedDictionaryNeedsNothingOnTheReadingSide(): Unit =
    val dictionary = ok(Ace.trainDictionary(samples, 2048))
    val config = small.copy(dictionary = Some(DictionaryUse(dictionary)))
    ok(Ace.use(config) { writer =>
      val packed = ok(writer.compressWithStats(input))
      assertEquals(6, packed.stats.formatMinor)
      assertTrue(packed.stats.dictionaryBlocks > 0)
      Ace.use() { plain =>
        assertArrayEquals(input, ok(plain.decompress(packed.bytes)))
        Right(())
      }
    })

  @Test def externalDictionaryNeedsTheReaderToSupplyIt(): Unit =
    val dictionary = ok(Ace.trainDictionary(samples, 2048))
    val config = small.copy(dictionary = Some(DictionaryUse(dictionary, DictionaryScope.External)))
    val packed = ok(Ace.use(config)(_.compress(input)))
    ok(Ace.use() { plain =>
      assertTrue(err(plain.decompress(packed)).isInstanceOf[AceError.Unsupported])
      Right(())
    })
    ok(Ace.use(dictionaries = Seq(dictionary)) { reader =>
      assertArrayEquals(input, ok(reader.decompress(packed)))
      val range = ok(reader.openRange(packed))
      try assertArrayEquals(input.slice(5, 25), ok(range.read(5, 25)))
      finally range.close()
      Right(())
    })

  @Test def schemaStoresATableColumnByColumn(): Unit =
    val schema = Schema.of(Column.unsigned(8), Column.float(8), Column.unsigned(8))
    val rows = table(20_000)
    ok(Ace.use(AceConfig(schema = Some(schema))) { columnar =>
      val packed = ok(columnar.compressWithStats(rows))
      assertTrue(packed.stats.columnarBlocks > 0)
      Ace.use() { plain =>
        assertTrue(packed.bytes.length < ok(plain.compress(rows)).length)
        assertArrayEquals(rows, ok(plain.decompress(packed.bytes)), "readers need no schema")
        Right(())
      }
    })

  @Test def segmentationNeverCostsSpace(): Unit =
    val mixed = Data.counters(400_000) ++ Data.random(1_500_000, 3)
    val config = AceConfig(blockSize = 4 << 20)
    val segmented = config.copy(segmentation = Some(Segmentation(minSegmentSize = 128 * 1024)))
    val whole = ok(Ace.use(config)(_.compress(mixed)))
    val packed = ok(Ace.use(segmented)(_.compress(mixed)))
    assertTrue(packed.length <= whole.length)
    assertArrayEquals(mixed, ok(Ace.use()(_.decompress(packed))))

  @Test def theCodecKnowsItsConfiguration(): Unit =
    val config = small.copy(segmentation = Some(Segmentation()))
    ok(Ace.use(config) { codec =>
      assertEquals(config, codec.config)
      Right(())
    })
