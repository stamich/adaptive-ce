package io.ace.sdk

import io.ace.{AceException, Engine, RangeReader as JReader}
import io.ace.model.*

import java.lang.foreign.MemorySegment
import java.nio.file.Path
import scala.util.control.NonFatal

/** Runs a native call and turns the library's failures into [[AceError]] values. */
def guarded[A](body: => A): Either[AceError, A] =
  try Right(body)
  catch
    case e: AceException                  => Left(Convert.error(e))
    case e: UnsatisfiedLinkError          => Left(AceError.Unavailable(e.getMessage))
    case e: ExceptionInInitializerError   => Left(AceError.Unavailable(String.valueOf(e.getCause)))
    case e: IllegalStateException         => Left(AceError.InvalidArgument(e.getMessage))
    case e: IllegalArgumentException      => Left(AceError.InvalidArgument(e.getMessage))
    case e: java.io.IOException           => Left(AceError.Io(String.valueOf(e.getMessage)))

/** Entry points of the SDK. */
object Ace:

  /** Opens a codec. The caller closes it; prefer [[Ace.use]]. */
  def open(config: AceConfig = AceConfig.Default, limits: DecodeLimits = DecodeLimits.Default): Either[AceError, AceCodec] =
    (config.validate ++ limits.validate) match
      case Nil => guarded(new AceCodec(Engine.create(Convert.config(config), Convert.limits(limits))))
      case problems => Left(AceError.InvalidArgument(problems.mkString("; ")))

  /** Opens a codec, runs `body` with it and always closes it. */
  def use[A](config: AceConfig = AceConfig.Default, limits: DecodeLimits = DecodeLimits.Default)(
      body: AceCodec => Either[AceError, A]
  ): Either[AceError, A] =
    open(config, limits).flatMap { codec =>
      try body(codec)
      finally codec.close()
    }

  /** The version of the native library. */
  def nativeVersion: Either[AceError, String] = guarded(io.ace.NativeLibrary.engineVersion())

/** An open engine. Safe to share between threads. */
final class AceCodec private[sdk] (private[sdk] val engine: Engine) extends AutoCloseable:

  /** The underlying Java engine, for adapters in other modules of the SDK. */
  def javaEngine: Engine = engine

  def config: AceConfig = scalaConfig

  private lazy val scalaConfig: AceConfig =
    val c = engine.config()
    AceConfig(
      profile = c.profile.ordinal match
        case 0 => Profile.Fast
        case 1 => Profile.Balanced
        case _ => Profile.Dense,
      blockSize = c.blockSize,
      threads = c.threads,
      memoryBudget = c.memoryBudget,
      sizePolicy = if c.blockSizePolicy.ordinal == 0 then SizePolicy.Fixed else SizePolicy.Auto,
      accessHint = c.accessHint.ordinal match
        case 0 => AccessHint.Sequential
        case 1 => AccessHint.Balanced
        case _ => AccessHint.RandomAccess,
      writeIndex = c.writeIndex,
      numericSpecialization = c.numericSpecialization,
      floatSpecialization = c.floatSpecialization
    )

  def compress(input: Array[Byte]): Either[AceError, Array[Byte]] = guarded(engine.compress(input))

  def compressWithStats(input: Array[Byte]): Either[AceError, CompressedResult] =
    guarded {
      val r = engine.compressWithStats(input)
      CompressedResult(r.bytes, Convert.stats(r.stats))
    }

  def decompress(file: Array[Byte]): Either[AceError, Array[Byte]] = guarded(engine.decompress(file))

  /** Decodes into `output` and returns the decoded size; a small buffer gives [[AceError.BufferTooSmall]]. */
  def decompressInto(file: MemorySegment, output: MemorySegment): Either[AceError, Long] =
    guarded(engine.decompressInto(file, output))

  /** Compresses off-heap input into an off-heap buffer the caller must close. */
  def compressOffHeap(input: MemorySegment): Either[AceError, OffHeapBytes] =
    guarded(new OffHeapBytes(engine.compress(input)))

  def decompressOffHeap(file: MemorySegment): Either[AceError, OffHeapBytes] =
    guarded(new OffHeapBytes(engine.decompress(file)))

  /** The planner's per-block report as JSON. Slow. */
  def explainJson(input: Array[Byte]): Either[AceError, String] = guarded(engine.explainJson(input))

  def openRange(file: Array[Byte]): Either[AceError, AceRange] = guarded(new AceRange(engine.openRange(file)))

  def openRange(file: Path): Either[AceError, AceRange] = guarded(new AceRange(engine.openRange(file)))

  def isClosed: Boolean = engine.isClosed

  override def close(): Unit = engine.close()

/** Bytes owned by the native library. Close it, or use [[OffHeapBytes.use]]. */
final class OffHeapBytes private[sdk] (private val buffer: io.ace.NativeBuffer) extends AutoCloseable:
  def segment: MemorySegment = buffer.segment()
  def size: Long = buffer.size()
  def toArray: Array[Byte] = buffer.toByteArray
  def use[A](f: OffHeapBytes => A): A =
    try f(this)
    finally close()
  override def close(): Unit = buffer.close()

/** Random access into one indexed ACE file. One thread at a time. */
final class AceRange private[sdk] (private val reader: JReader) extends AutoCloseable:
  def length: Long = reader.length()
  def blockCount: Long = reader.blockCount()
  def read(start: Long, end: Long): Either[AceError, Array[Byte]] = guarded(reader.read(start, end))
  def readBlock(block: Long): Either[AceError, Array[Byte]] = guarded(reader.readBlock(block))
  def lastStats: RangeReadStats = Convert.rangeStats(reader.lastStats())
  override def close(): Unit = reader.close()
