package io.ace.sdk

import io.ace.model.*
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

import java.nio.file.{Files, Path}
import java.security.MessageDigest
import java.util.HexFormat

/** The Scala SDK writes exactly the bytes of `examples/golden/0.6.1/GOLDEN.json`. */
class GoldenTest:

  private val WorkloadBytes = 1024L * 1024

  private final case class Expected(profile: String, sha256: String, bytes: Long)
  private final case class Entry(workload: String, inputSha256: String, profiles: List[Expected])

  private def sha256(bytes: Array[Byte]): String =
    HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes))

  /** The golden file is pretty-printed by a tool; this reads only the members the test needs. */
  private def entries(): List[Entry] =
    val root = Path.of(System.getProperty("ace.repo.root"))
    val text = Files.readString(root.resolve("examples/golden/0.6.1/GOLDEN.json"))
    val workload = """"workload"\s*:\s*"([^"]+)"""".r
    val inputDigest = """"input_sha256"\s*:\s*"([0-9a-f]{64})"""".r
    val profile =
      """"profile"\s*:\s*"([a-z]+)"\s*,\s*"ace_sha256"\s*:\s*"([0-9a-f]{64})"\s*,\s*"ace_bytes"\s*:\s*(\d+)""".r
    text.split("""(?="workload")""").toList.filter(_.contains("\"workload\"")).map { block =>
      Entry(
        workload.findFirstMatchIn(block).get.group(1),
        inputDigest.findFirstMatchIn(block).get.group(1),
        profile.findAllMatchIn(block).map(m => Expected(m.group(1), m.group(2), m.group(3).toLong)).toList
      )
    }

  private def generate(workload: String): Array[Byte] =
    val binary = Option(System.getProperty("ace.corpus.bin")).map(Path.of(_))
    assertTrue(binary.exists(Files.isExecutable), s"ace.corpus.bin must name the ace-corpus binary, got $binary")
    val file = Files.createTempFile("ace-golden-", ".bin")
    try
      val process = new ProcessBuilder(binary.get.toString, workload, WorkloadBytes.toString, file.toString)
        .redirectErrorStream(true)
        .start()
      val output = new String(process.getInputStream.readAllBytes())
      assertEquals(0, process.waitFor(), output)
      Files.readAllBytes(file)
    finally Files.deleteIfExists(file)

  private def profileOf(name: String): Profile = name match
    case "fast"     => Profile.Fast
    case "balanced" => Profile.Balanced
    case "dense"    => Profile.Dense
    case other      => throw new IllegalArgumentException(other)

  @Test def reproducesTheGoldenFileForEveryWorkloadAndProfile(): Unit =
    val all = entries()
    assertEquals(31, all.size)
    all.foreach { entry =>
      val input = generate(entry.workload)
      assertEquals(entry.inputSha256, sha256(input), s"${entry.workload} input")
      assertEquals(3, entry.profiles.size, entry.workload)
      entry.profiles.foreach { expected =>
        val label = s"${entry.workload} ${expected.profile}"
        val result = Ace.use(AceConfig(profile = profileOf(expected.profile))) { codec =>
          codec.compress(input).flatMap { packed =>
            codec.decompress(packed).map(restored => (packed, restored))
          }
        }
        result match
          case Right((packed, restored)) =>
            assertEquals(expected.bytes, packed.length.toLong, label)
            assertEquals(expected.sha256, sha256(packed), label)
            assertArrayEquals(input, restored, label)
          case Left(error) => fail(s"$label: $error")
      }
    }

  @Test def theBytesDoNotDependOnTheThreadCount(): Unit =
    val entry = entries().find(_.workload == "low-cardinality").get
    val expected = entry.profiles.find(_.profile == "balanced").get.sha256
    val input = generate("low-cardinality")
    for threads <- List(1L, 2L, 3L, 8L) do
      Ace.use(AceConfig(threads = threads))(_.compress(input)) match
        case Right(packed) => assertEquals(expected, sha256(packed), s"$threads threads")
        case Left(error)   => fail(s"$threads threads: $error")
