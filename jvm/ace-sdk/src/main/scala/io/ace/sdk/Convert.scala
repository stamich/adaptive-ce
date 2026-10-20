package io.ace.sdk

import io.ace.{AceException, BufferTooSmallException, Status}
import io.ace.{AccessHint => JAccess, BlockSizePolicy => JPolicy, Config => JConfig}
import io.ace.{DecodeLimits => JLimits, Profile => JProfile, RangeStats => JRange, Stats => JStats}
import io.ace.model.*

import scala.jdk.CollectionConverters.*

/** Translation between the pure Scala model and the Java SDK. */
object Convert:

  def config(c: AceConfig): JConfig =
    JConfig
      .builder()
      .profile(c.profile match
        case Profile.Fast     => JProfile.FAST
        case Profile.Balanced => JProfile.BALANCED
        case Profile.Dense    => JProfile.DENSE)
      .blockSize(c.blockSize)
      .threads(c.threads)
      .memoryBudget(c.memoryBudget)
      .blockSizePolicy(c.sizePolicy match
        case SizePolicy.Fixed => JPolicy.FIXED
        case SizePolicy.Auto  => JPolicy.AUTO)
      .accessHint(c.accessHint match
        case AccessHint.Sequential   => JAccess.SEQUENTIAL
        case AccessHint.Balanced     => JAccess.BALANCED
        case AccessHint.RandomAccess => JAccess.RANDOM_ACCESS)
      .writeIndex(c.writeIndex)
      .numericSpecialization(c.numericSpecialization)
      .floatSpecialization(c.floatSpecialization)
      .build()

  def limits(l: DecodeLimits): JLimits =
    JLimits
      .builder()
      .maxOutputSize(l.maxOutputSize)
      .maxBlockSize(l.maxBlockSize)
      .maxEncodedBlockSize(l.maxEncodedBlockSize)
      .maxTransforms(l.maxTransforms)
      .maxDictionarySize(l.maxDictionarySize)
      .maxBlocks(l.maxBlocks)
      .build()

  def stats(s: JStats): CompressionStats =
    CompressionStats(
      s.inputBytes,
      s.outputBytes,
      s.formatMinor,
      s.blocks,
      s.rawBlocks,
      s.rleBlocks,
      s.lzBlocks,
      s.numericBlocks,
      s.timeSeriesBlocks,
      s.ratio,
      s.planDistribution.asScala.view.mapValues(_.longValue).toMap
    )

  def rangeStats(s: JRange): RangeReadStats = RangeReadStats(s.logicalBytes, s.physicalBytes, s.blocks)

  def error(e: AceException): AceError =
    val block = if e.blockId.isPresent then Some(e.blockId.getAsLong) else None
    val m = Option(e.getMessage).getOrElse("")
    e match
      case b: BufferTooSmallException => AceError.BufferTooSmall(m, b.requiredSize)
      case _ =>
        e.status match
          case Status.INVALID_ARGUMENT   => AceError.InvalidArgument(m)
          case Status.NOT_ACE_FILE       => AceError.NotAceFile(m)
          case Status.UNSUPPORTED        => AceError.Unsupported(m)
          case Status.CORRUPT            => AceError.Corrupt(m)
          case Status.CHECKSUM_MISMATCH  => AceError.ChecksumMismatch(m, block)
          case Status.LIMIT_EXCEEDED     => AceError.LimitExceeded(m)
          case Status.BLOCK_NOT_FOUND    => AceError.BlockNotFound(m, block)
          case Status.IO                 => AceError.Io(m)
          case Status.BUFFER_TOO_SMALL   => AceError.BufferTooSmall(m, 0L)
          case Status.PANIC | Status.INTERNAL => AceError.Internal(m)
          case _                         => AceError.Unknown(m, e.rawStatus)

  /** Maps any throwable of a native call to an error; other throwables are rethrown. */
  def failure[A](t: Throwable): Either[AceError, A] =
    guarded[A](throw t)
