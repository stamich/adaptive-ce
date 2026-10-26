package io.ace.schema

import io.ace.model.{AceDictionary, AceError}

import java.io.IOException
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import java.nio.file.{Files, Path, StandardCopyOption}
import java.util.concurrent.ConcurrentHashMap
import java.util.zip.CRC32
import scala.jdk.CollectionConverters.*
import scala.jdk.StreamConverters.*

/**
 * Keeps the dictionaries a system uses, by id, so that readers of files written with an external
 * dictionary can find it. A file names its dictionary by id only; the store is where the bytes
 * come from.
 *
 * {{{
 * val store = DictionaryStore.directory(Path.of("/var/lib/app/dictionaries"))
 * store.put(dictionary)
 * Ace.open(config, dictionaries = store.all.getOrElse(Nil))
 * }}}
 *
 * Implementations are thread-safe.
 */
trait DictionaryStore:
  /** Stores `dictionary`; storing the same dictionary again changes nothing. */
  def put(dictionary: AceDictionary): Either[AceError, Unit]

  /** The dictionary with this id, if the store has it. */
  def get(id: Long): Either[AceError, Option[AceDictionary]]

  /** Every dictionary in the store, ordered by id. */
  def all: Either[AceError, Seq[AceDictionary]]

  /** Removes the dictionary with this id; true when there was one. Files written with it become unreadable. */
  def remove(id: Long): Either[AceError, Boolean]

object DictionaryStore:
  /** A store that lives in memory only; for tests and short-lived processes. */
  def inMemory: DictionaryStore = new InMemoryDictionaryStore

  /**
   * A store in a directory, one file per dictionary, created if needed. Files are written to a
   * temporary name and moved into place, so a crash never leaves a half-written dictionary, and
   * each carries a CRC-32 that is checked when it is read.
   */
  def directory(path: Path): DictionaryStore = new DirectoryDictionaryStore(path)

private final class InMemoryDictionaryStore extends DictionaryStore:
  private val entries = new ConcurrentHashMap[Long, AceDictionary]()

  def put(dictionary: AceDictionary): Either[AceError, Unit] =
    dictionary.validate match
      case Nil      => entries.put(dictionary.id, dictionary); Right(())
      case problems => Left(AceError.InvalidArgument(problems.mkString("; ")))

  def get(id: Long): Either[AceError, Option[AceDictionary]] = Right(Option(entries.get(id)))

  def all: Either[AceError, Seq[AceDictionary]] =
    Right(entries.values.asScala.toSeq.sortWith((a, b) => java.lang.Long.compareUnsigned(a.id, b.id) < 0))

  def remove(id: Long): Either[AceError, Boolean] = Right(entries.remove(id) != null)

/**
 * File layout (little-endian): the 8 ASCII bytes `ACEDICT1`, the id (u64), the length (u32), the
 * CRC-32 of the dictionary bytes (u32), then the bytes.
 */
private final class DirectoryDictionaryStore(directory: Path) extends DictionaryStore:
  private val Magic = "ACEDICT1".getBytes(StandardCharsets.US_ASCII)
  private val HeaderSize = 8 + 8 + 4 + 4
  private val Suffix = ".acedict"

  private def fileOf(id: Long): Path = directory.resolve(f"$id%016x$Suffix")

  private def io[A](body: => A): Either[AceError, A] =
    try Right(body)
    catch case e: IOException => Left(AceError.Io(String.valueOf(e.getMessage)))

  private def crc(bytes: Array[Byte]): Long =
    val c = new CRC32
    c.update(bytes)
    c.getValue

  def put(dictionary: AceDictionary): Either[AceError, Unit] =
    dictionary.validate match
      case problems if problems.nonEmpty => Left(AceError.InvalidArgument(problems.mkString("; ")))
      case _ =>
        io {
          Files.createDirectories(directory)
          val buffer = ByteBuffer.allocate(HeaderSize + dictionary.size).order(ByteOrder.LITTLE_ENDIAN)
          buffer.put(Magic).putLong(dictionary.id).putInt(dictionary.size).putInt(crc(dictionary.bytes).toInt)
          buffer.put(dictionary.bytes)
          val temporary = Files.createTempFile(directory, "dictionary-", ".tmp")
          try
            Files.write(temporary, buffer.array())
            Files.move(temporary, fileOf(dictionary.id), StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE)
          finally Files.deleteIfExists(temporary)
          ()
        }

  def get(id: Long): Either[AceError, Option[AceDictionary]] =
    val file = fileOf(id)
    if !Files.exists(file) then Right(None)
    else read(file).filterOrElse(_.id == id, AceError.Corrupt(s"$file holds another dictionary")).map(Some(_))

  def all: Either[AceError, Seq[AceDictionary]] =
    if !Files.isDirectory(directory) then Right(Nil)
    else
      io(Files.list(directory).toScala(List).filter(_.getFileName.toString.endsWith(Suffix)).sortBy(_.getFileName.toString))
        .flatMap(_.foldLeft[Either[AceError, List[AceDictionary]]](Right(Nil)) { (acc, file) =>
          for
            done <- acc
            one <- read(file)
          yield done :+ one
        })

  def remove(id: Long): Either[AceError, Boolean] = io(Files.deleteIfExists(fileOf(id)))

  private def read(file: Path): Either[AceError, AceDictionary] =
    io(Files.readAllBytes(file)).flatMap { raw =>
      if raw.length < HeaderSize || !java.util.Arrays.equals(raw.take(Magic.length), Magic) then
        Left(AceError.Corrupt(s"$file is not a dictionary file"))
      else
        val header = ByteBuffer.wrap(raw, Magic.length, HeaderSize - Magic.length).order(ByteOrder.LITTLE_ENDIAN)
        val id = header.getLong()
        val length = header.getInt()
        val expected = header.getInt().toLong & 0xffffffffL
        if length < 0 || length != raw.length - HeaderSize then Left(AceError.Corrupt(s"$file has a wrong length"))
        else
          val bytes = java.util.Arrays.copyOfRange(raw, HeaderSize, raw.length)
          if crc(bytes) != expected then Left(AceError.ChecksumMismatch(s"$file failed its checksum", None))
          else Right(AceDictionary(id, bytes))
    }
