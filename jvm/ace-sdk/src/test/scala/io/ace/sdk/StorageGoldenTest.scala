package io.ace.sdk

import io.ace.model.*
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

import java.nio.charset.StandardCharsets
import java.nio.file.{Files, Path}
import java.nio.{ByteBuffer, ByteOrder}
import java.security.MessageDigest
import java.util.HexFormat

/**
 * The Scala SDK writes exactly the bytes of `examples/golden/0.8.0/GOLDEN.json`: columnar blocks
 * from a schema, segments, and embedded and external dictionaries trained through
 * [[Ace.trainDictionary]], for every profile. Record inputs come from `ace-corpus`, the others
 * are generated here; every input is checked against its digest first.
 */
class StorageGoldenTest:

  private def sha256(bytes: Array[Byte]): String =
    HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes))

  /** The file is flat (scalars only), so a regex per member is enough. */
  private def cases(): List[Map[String, String]] =
    val root = Path.of(System.getProperty("ace.repo.root"))
    val text = Files.readString(root.resolve("examples/golden/0.8.0/GOLDEN.json"))
    val member = """"(\w+)"\s*:\s*(?:"([^"]*)"|(\d+))""".r
    text.split("""(?=\{\s*"case")""").toList.filter(_.contains("\"case\"")).map { block =>
      member.findAllMatchIn(block).map(m => m.group(1) -> Option(m.group(2)).getOrElse(m.group(3))).toMap
    }

  private def jsonRecord(i: Int): Array[Byte] =
    s"""{"sensor":"temperature_reading","unit":"celsius","site":"plant-${i % 7}","value":${1000 + i * 13 % 97}}
""".getBytes(StandardCharsets.UTF_8)

  private def corpus(workload: String, bytes: String): Array[Byte] =
    val binary = Option(System.getProperty("ace.corpus.bin")).map(Path.of(_))
    assertTrue(binary.exists(Files.isExecutable), s"ace.corpus.bin must name the ace-corpus binary, got $binary")
    val file = Files.createTempFile("ace-golden-", ".bin")
    try
      val process = new ProcessBuilder(binary.get.toString, workload, bytes, file.toString)
        .redirectErrorStream(true)
        .start()
      val output = new String(process.getInputStream.readAllBytes())
      assertEquals(0, process.waitFor(), output)
      Files.readAllBytes(file)
    finally Files.deleteIfExists(file)

  private def input(generator: String): Array[Byte] =
    if generator.startsWith("records-") then
      val Array(name, size) = generator.split(":")
      corpus(name, size)
    else if generator.startsWith("json-records:") then
      (0 until generator.stripPrefix("json-records:").toInt).flatMap(i => jsonRecord(i).toSeq).toArray
    else
      assertEquals("mixed-regimes", generator)
      val b = ByteBuffer.allocate(400_000 * 4 + 1_500_000).order(ByteOrder.LITTLE_ENDIAN)
      var i = 0
      while i < 400_000 do { b.putInt(i / 2); i += 1 }
      var j = 0L
      while j < 1_500_000L do { b.put(((j * 2_654_435_761L) >>> 13).toByte); j += 1 }
      b.array()

  private def schema(layout: String): Schema =
    Schema(layout.split(",").toList.map { column =>
      val width = column.drop(1).toInt
      column.head match
        case 'u' => Column.unsigned(width)
        case 's' => Column.signed(width)
        case 'f' => Column.float(width)
        case 'b' => Column.bytes(width)
    })

  private def profileOf(name: String): Profile = name match
    case "fast"     => Profile.Fast
    case "balanced" => Profile.Balanced
    case "dense"    => Profile.Dense

  @Test def reproducesTheStorageGoldenFileForEveryCaseAndProfile(): Unit =
    val all = cases()
    assertEquals(6, all.size)
    all.foreach { entry =>
      val name = entry("case")
      val data = input(entry("generator"))
      assertEquals(entry("input_sha256"), sha256(data), s"$name: generator")

      val trained = Option.when(entry("dictionary") != "none") {
        Ace.trainDictionary((0 until 300).map(jsonRecord), 2048) match
          case Right(d) => d
          case Left(e)  => fail(s"training failed: $e").asInstanceOf[AceDictionary]
      }
      trained.foreach { d =>
        assertEquals(entry("dictionary_id"), f"${d.id}%016x", name)
        assertEquals(entry("dictionary_sha256"), sha256(d.bytes), s"$name: trained dictionary")
      }
      val scope = if entry("dictionary") == "external" then DictionaryScope.External else DictionaryScope.Embedded
      for profile <- List("fast", "balanced", "dense") do
        val config = AceConfig(
          profile = profileOf(profile),
          blockSize = entry("block_size").toLong,
          schema = Option.when(entry("schema").nonEmpty)(schema(entry("schema"))),
          dictionary = trained.map(DictionaryUse(_, scope)),
          segmentation = Option.when(entry("segmentation_min").toLong > 0)(
            Segmentation(minSegmentSize = entry("segmentation_min").toLong)
          )
        )
        Ace.use(config) { codec =>
          for
            file <- codec.compress(data)
            back <- codec.decompress(file)
          yield
            assertEquals(entry(s"${profile}_bytes").toInt, file.length, s"$name $profile")
            assertEquals(entry(s"${profile}_sha256"), sha256(file), s"$name $profile: bytes")
            assertArrayEquals(data, back, s"$name $profile: round trip")
        } match
          case Right(_) => ()
          case Left(e)  => fail(s"$name $profile: $e")
    }
