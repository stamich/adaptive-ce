package io.ace.model

/** Compression effort. */
enum Profile:
  case Fast, Balanced, Dense

/** Whether the block size is used as given or chosen once per file. */
enum SizePolicy:
  case Fixed, Auto

/** The expected way a file will be read. */
enum AccessHint:
  case Sequential, Balanced, RandomAccess

/** What a [[Column]] holds. The kind steers the codec tried for the column, never how a file is read. */
enum ColumnKind:
  /** Opaque bytes of any width: identifiers, text, flags. */
  case Bytes
  /** Unsigned little-endian integers of width 1, 2, 4 or 8. */
  case Unsigned
  /** Signed little-endian integers of width 1, 2, 4 or 8. */
  case Signed
  /** IEEE-754 little-endian floats of width 4 or 8. */
  case Float

/** One field of a fixed-size record: `width` bytes of a given kind. */
final case class Column(kind: ColumnKind, width: Int):
  /** The problem with this column, if there is one. */
  def problem: Option[String] =
    if width < 1 || width > 65535 then Some(s"column width must be 1 to 65535: $width")
    else
      kind match
        case ColumnKind.Unsigned | ColumnKind.Signed if !Set(1, 2, 4, 8).contains(width) =>
          Some(s"$kind columns cannot be $width bytes wide")
        case ColumnKind.Float if width != 4 && width != 8 => Some(s"Float columns cannot be $width bytes wide")
        case _                                          => None

object Column:
  def bytes(width: Int): Column = Column(ColumnKind.Bytes, width)
  def unsigned(width: Int): Column = Column(ColumnKind.Unsigned, width)
  def signed(width: Int): Column = Column(ColumnKind.Signed, width)
  def float(width: Int): Column = Column(ColumnKind.Float, width)

/**
 * The record layout of a table, in record order. With a schema ACE also tries storing every block
 * column by column, which usually compresses much better; a block keeps that form only when it is
 * smaller. Readers need no schema.
 */
final case class Schema(columns: List[Column]):
  /** Size of one record in bytes. */
  def recordSize: Int = columns.map(_.width).sum

  def validate: List[String] =
    if columns.isEmpty then List("a schema needs at least one column") else columns.flatMap(_.problem)

object Schema:
  def of(columns: Column*): Schema = Schema(columns.toList)

/**
 * Lets blocks be split into segments with different codecs. A value of 0 keeps the library
 * default (16 KiB window, 64 KiB to 1 MiB segments, 1 024 segments per block).
 */
final case class Segmentation(
    windowSize: Long = 0L,
    minSegmentSize: Long = 0L,
    maxSegmentSize: Long = 0L,
    maxSegments: Long = 0L
):
  def validate: List[String] =
    List(
      "windowSize" -> windowSize,
      "minSegmentSize" -> minSegmentSize,
      "maxSegmentSize" -> maxSegmentSize,
      "maxSegments" -> maxSegments
    ).collect { case (name, v) if v < 0 => s"$name must not be negative" }

/**
 * A preset dictionary: bytes every block is compressed "after", for small inputs that share a
 * vocabulary. The `id` is what files store; it is never 0.
 */
final case class AceDictionary(id: Long, bytes: Array[Byte]):
  def size: Int = bytes.length

  def validate: List[String] =
    List(
      Option.when(id == 0L)("a dictionary id must not be 0"),
      Option.when(bytes.isEmpty || bytes.length > AceDictionary.MaxSize)(
        s"a dictionary must be 1 to ${AceDictionary.MaxSize} bytes"
      )
    ).flatten

  // Arrays compare by reference; a dictionary is the same dictionary when id and content match.
  override def equals(other: Any): Boolean = other match
    case that: AceDictionary => id == that.id && java.util.Arrays.equals(bytes, that.bytes)
    case _                   => false

  override def hashCode: Int = 31 * java.lang.Long.hashCode(id) + java.util.Arrays.hashCode(bytes)

  override def toString: String = f"AceDictionary(id=$id%016x, size=$size)"

object AceDictionary:
  /** Largest dictionary the format allows, in bytes. */
  val MaxSize: Int = 65535

/** Where a dictionary lives. */
enum DictionaryScope:
  /** Stored in the file when that pays; the file is self-contained. */
  case Embedded
  /** Only the id is stored; every reader needs the same dictionary. */
  case External

/** The dictionary a writer uses, and how files refer to it. */
final case class DictionaryUse(dictionary: AceDictionary, scope: DictionaryScope = DictionaryScope.Embedded)

/**
 * Encoder settings. A value of 0 for a size keeps the library default.
 *
 * The last three members are Format 1.6 features and are off by default, so a default
 * configuration writes the same bytes as 0.7.0.
 */
final case class AceConfig(
    profile: Profile = Profile.Balanced,
    blockSize: Long = 0L,
    threads: Long = 0L,
    memoryBudget: Long = 0L,
    sizePolicy: SizePolicy = SizePolicy.Fixed,
    accessHint: AccessHint = AccessHint.Balanced,
    writeIndex: Boolean = true,
    numericSpecialization: Boolean = true,
    floatSpecialization: Boolean = true,
    schema: Option[Schema] = None,
    dictionary: Option[DictionaryUse] = None,
    segmentation: Option[Segmentation] = None
):
  /** The problems with these settings; empty when they are usable. */
  def validate: List[String] =
    List(
      Option.when(blockSize < 0)("blockSize must not be negative"),
      Option.when(threads < 0)("threads must not be negative"),
      Option.when(memoryBudget < 0)("memoryBudget must not be negative")
    ).flatten ++ schema.toList.flatMap(_.validate) ++ dictionary.toList.flatMap(_.dictionary.validate) ++
      segmentation.toList.flatMap(_.validate)

object AceConfig:
  val Default: AceConfig = AceConfig()

/** Decode limits. A value of 0 keeps the library default. */
final case class DecodeLimits(
    maxOutputSize: Long = 0L,
    maxBlockSize: Long = 0L,
    maxEncodedBlockSize: Long = 0L,
    maxTransforms: Long = 0L,
    maxDictionarySize: Long = 0L,
    maxBlocks: Long = 0L
):
  def validate: List[String] =
    List(
      "maxOutputSize" -> maxOutputSize,
      "maxBlockSize" -> maxBlockSize,
      "maxEncodedBlockSize" -> maxEncodedBlockSize,
      "maxTransforms" -> maxTransforms,
      "maxDictionarySize" -> maxDictionarySize,
      "maxBlocks" -> maxBlocks
    ).collect { case (name, v) if v < 0 => s"$name must not be negative" }

object DecodeLimits:
  val Default: DecodeLimits = DecodeLimits()

/** What a compression run did. */
final case class CompressionStats(
    inputBytes: Long,
    outputBytes: Long,
    formatMinor: Int,
    blocks: Long,
    rawBlocks: Long,
    rleBlocks: Long,
    lzBlocks: Long,
    numericBlocks: Long,
    timeSeriesBlocks: Long,
    ratio: Double,
    planDistribution: Map[String, Long],
    segmentedBlocks: Long = 0L,
    columnarBlocks: Long = 0L,
    dictionaryBlocks: Long = 0L
):
  /** Share of the input that the output saved, between 0 and 1 (negative if it grew). */
  def savings: Double = if inputBytes == 0 then 0.0 else 1.0 - outputBytes.toDouble / inputBytes

/** The work of one range read. */
final case class RangeReadStats(logicalBytes: Long, physicalBytes: Long, blocks: Long):
  def readAmplification: Double =
    if logicalBytes == 0 then 0.0 else physicalBytes.toDouble / logicalBytes

/** Compressed bytes together with the statistics of the run that produced them. */
final case class CompressedResult(bytes: Array[Byte], stats: CompressionStats)

/** Why an operation failed. The cases mirror the status codes of the C ABI. */
enum AceError(val message: String):
  case InvalidArgument(override val message: String) extends AceError(message)
  case NotAceFile(override val message: String) extends AceError(message)
  case Unsupported(override val message: String) extends AceError(message)
  case Corrupt(override val message: String) extends AceError(message)
  case ChecksumMismatch(override val message: String, block: Option[Long]) extends AceError(message)
  case LimitExceeded(override val message: String) extends AceError(message)
  case BlockNotFound(override val message: String, block: Option[Long]) extends AceError(message)
  case Io(override val message: String) extends AceError(message)
  case BufferTooSmall(override val message: String, required: Long) extends AceError(message)
  /** A bug in the library: a caught panic or an impossible state. */
  case Internal(override val message: String) extends AceError(message)
  /** The library is newer than this SDK and returned a status it does not know. */
  case Unknown(override val message: String, code: Int) extends AceError(message)
  /** The native library could not be loaded or has the wrong ABI version. */
  case Unavailable(override val message: String) extends AceError(message)

  /** True when retrying the same call cannot succeed: the data or the arguments are at fault. */
  def isPermanent: Boolean = this match
    case Io(_) | Internal(_) | Unavailable(_) => false
    case _                                    => true

  /** True when the input is damaged, as opposed to the caller misusing the API. */
  def isDataError: Boolean = this match
    case NotAceFile(_) | Corrupt(_) | ChecksumMismatch(_, _) | Unsupported(_) => true
    case _                                                                      => false
