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

/** Encoder settings. A value of 0 for a size keeps the library default. */
final case class AceConfig(
    profile: Profile = Profile.Balanced,
    blockSize: Long = 0L,
    threads: Long = 0L,
    memoryBudget: Long = 0L,
    sizePolicy: SizePolicy = SizePolicy.Fixed,
    accessHint: AccessHint = AccessHint.Balanced,
    writeIndex: Boolean = true,
    numericSpecialization: Boolean = true,
    floatSpecialization: Boolean = true
):
  /** The problems with these settings; empty when they are usable. */
  def validate: List[String] =
    List(
      Option.when(blockSize < 0)("blockSize must not be negative"),
      Option.when(threads < 0)("threads must not be negative"),
      Option.when(memoryBudget < 0)("memoryBudget must not be negative")
    ).flatten

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
    planDistribution: Map[String, Long]
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
