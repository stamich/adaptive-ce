package io.ace.demo

import io.ace.model.*
import io.ace.sdk.*
import io.ace.stream.AceInputStream
import io.ace.tier.TierCodec

import java.nio.{ByteBuffer, ByteOrder}

/** ACE from Scala 3: Either results, a scoped codec, a stream view and the tier metrics. */
object ScalaDemo:

  private def timestamps(count: Int): Array[Byte] =
    val buffer = ByteBuffer.allocate(count * 8).order(ByteOrder.LITTLE_ENDIAN)
    var time = 1_700_000_000_000_000_000L
    for i <- 0 until count do
      time += 1_000_000L + (i * 7919L) % 200
      buffer.putLong(time)
    buffer.array()

  /** Runs the demo and returns what it prints, or the first error. */
  def run(): Either[AceError, String] =
    val input = timestamps(500_000)
    val out = new StringBuilder
    for
      _ <- Ace.use(AceConfig(profile = Profile.Balanced, blockSize = 256 * 1024)) { codec =>
        for
          packed <- codec.compressWithStats(input)
          restored <- codec.decompress(packed.bytes)
          _ <- Either.cond(restored.sameElements(input), (), AceError.Internal("round trip mismatch"))
          range <- codec.openRange(packed.bytes)
        yield
          out ++= f"output  ${packed.bytes.length}%,d bytes (ratio ${packed.stats.ratio}%.2f)%n"
          val stream = new AceInputStream(range)
          try out ++= f"stream  first byte ${stream.read()}%d of ${range.length}%,d%n"
          finally stream.close()
      }
      tiers <- TierCodec.open().flatMap { tierCodec =>
        try tierCodec.measureAll(input)
        finally tierCodec.close()
      }
    yield
      tiers.foreach { m =>
        out ++= f"${m.tier}%-5s ratio ${m.ratio}%.2f  encode ${m.encodeNsPerByte}%.2f ns/B  decode ${m.decodeNsPerByte}%.2f ns/B%n"
      }
      out.toString

  def main(args: Array[String]): Unit =
    run() match
      case Right(text) => print(text)
      case Left(error) =>
        System.err.println(s"ace: ${error.message}")
        sys.exit(1)
