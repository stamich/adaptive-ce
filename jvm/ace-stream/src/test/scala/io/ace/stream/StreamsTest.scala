package io.ace.stream

import io.ace.model.*
import io.ace.sdk.*
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

import java.io.{ByteArrayInputStream, ByteArrayOutputStream, IOException, InputStream, OutputStream}
import java.nio.{ByteBuffer, ByteOrder}

class StreamsTest:

  private val small = AceConfig(blockSize = 32 * 1024)

  private def counters(count: Int): Array[Byte] =
    val b = ByteBuffer.allocate(count * 4).order(ByteOrder.LITTLE_ENDIAN)
    (0 until count).foreach(i => b.putInt(i / 3))
    b.array()

  private def ok[A](r: Either[AceError, A]): A = r match
    case Right(a) => a
    case Left(e)  => fail(s"unexpected error: $e").asInstanceOf[A]

  @Test def streamingEqualsOneShot(): Unit =
    val input = counters(250_000)
    ok(Ace.use(small) { codec =>
      val packed = new ByteArrayOutputStream()
      val stats = ok(AceStreams.compress(codec, input.length.toLong, new ByteArrayInputStream(input), packed))
      assertArrayEquals(ok(codec.compress(input)), packed.toByteArray)
      assertEquals(input.length.toLong, stats.inputBytes)
      val restored = new ByteArrayOutputStream()
      ok(AceStreams.decompress(codec, new ByteArrayInputStream(packed.toByteArray), restored))
      assertArrayEquals(input, restored.toByteArray)
      Right(())
    })

  @Test def callbackFailuresBecomeIoErrors(): Unit =
    val input = counters(100_000)
    val broken = new InputStream:
      override def read(): Int = throw new IOException("disk on fire")
      override def read(b: Array[Byte], off: Int, len: Int): Int = throw new IOException("disk on fire")
    ok(Ace.use(small) { codec =>
      val r = AceStreams.compress(codec, input.length.toLong, broken, new ByteArrayOutputStream())
      assertTrue(r.left.exists(_.isInstanceOf[AceError.Io]), r.toString)
      Right(())
    })

  @Test def inputStreamReadsBlocksInOrder(): Unit =
    val input = counters(150_000)
    ok(Ace.use(small) { codec =>
      val packed = ok(codec.compress(input))
      val stream = new AceInputStream(ok(codec.openRange(packed)))
      try
        assertArrayEquals(input, stream.readAllBytes())
        assertEquals(-1, stream.read())
      finally stream.close()
      val again = new AceInputStream(ok(codec.openRange(packed)))
      try
        val first = new Array[Byte](10)
        assertEquals(10, again.read(first, 0, 10))
        assertArrayEquals(input.take(10), first)
        assertEquals((input(10) & 0xff), again.read())
      finally again.close()
      Right(())
    })

  @Test def blocksIterator(): Unit =
    val input = counters(150_000)
    ok(Ace.use(small) { codec =>
      val range = ok(codec.openRange(ok(codec.compress(input))))
      try
        val blocks = AceStreams.blocks(range).toList
        assertEquals(range.blockCount.toInt, blocks.size)
        assertArrayEquals(input, blocks.flatten.toArray)
        assertThrows(classOf[NoSuchElementException], () => AceStreams.blocks(range).drop(blocks.size).next())
      finally range.close()
      Right(())
    })

  @Test def outputStreamCompressesOnClose(): Unit =
    val input = counters(120_000)
    ok(Ace.use(small) { codec =>
      val target = new ByteArrayOutputStream()
      val out = new AceOutputStream(codec, target)
      input.grouped(10_000).foreach(chunk => out.write(chunk))
      assertEquals(0, target.size())
      out.close()
      out.close()
      assertEquals(Some(input.length.toLong), out.stats.map(_.inputBytes))
      assertArrayEquals(input, ok(codec.decompress(target.toByteArray)))
      assertThrows(classOf[IOException], () => out.write(1))
      Right(())
    })
