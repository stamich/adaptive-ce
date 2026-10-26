package io.ace.tier

import io.ace.model.*
import io.ace.sdk.*

/** A storage tier, from the data read most to the data read least. */
enum Tier:
  case Hot, Warm, Cold

/** The encoder settings used for each tier. The defaults trade size for speed on hot data. */
final case class TierPolicy(hot: AceConfig, warm: AceConfig, cold: AceConfig):
  def configFor(tier: Tier): AceConfig = tier match
    case Tier.Hot  => hot
    case Tier.Warm => warm
    case Tier.Cold => cold

  /** Changes the settings of one tier. */
  def withTier(tier: Tier, config: AceConfig): TierPolicy = tier match
    case Tier.Hot  => copy(hot = config)
    case Tier.Warm => copy(warm = config)
    case Tier.Cold => copy(cold = config)

object TierPolicy:
  /**
   * Hot data is read often and in small ranges: fast encoding, small blocks, an index.
   * Warm data is the library default. Cold data is written once and scanned rarely: dense
   * encoding and large blocks.
   */
  val Default: TierPolicy = TierPolicy(
    hot = AceConfig(profile = Profile.Fast, blockSize = 64 * 1024, accessHint = AccessHint.RandomAccess),
    warm = AceConfig(profile = Profile.Balanced, blockSize = 256 * 1024),
    cold = AceConfig(profile = Profile.Dense, blockSize = 1024 * 1024, accessHint = AccessHint.Sequential)
  )

/**
 * What a tier-economics evaluator needs from one run, with no dependency on any database:
 * sizes, the ratio and the time the codec took per input byte.
 */
final case class TierMetrics(
    tier: Tier,
    inputBytes: Long,
    outputBytes: Long,
    ratio: Double,
    encodeNsPerByte: Double,
    decodeNsPerByte: Double,
    blocks: Long
):
  /** Bytes the tier stores for each input byte (the inverse of the ratio). */
  def storedFraction: Double = if inputBytes == 0 then 1.0 else outputBytes.toDouble / inputBytes

  /** Nanoseconds spent decoding to serve `bytes` of original data. */
  def decodeCostNs(bytes: Long): Double = decodeNsPerByte * bytes

object TierMetrics:
  /** Builds the metrics from the statistics of a run and its measured durations. */
  def fromStats(tier: Tier, stats: CompressionStats, encodeNanos: Long, decodeNanos: Long): TierMetrics =
    val perByte = (nanos: Long) => if stats.inputBytes == 0 then 0.0 else nanos.toDouble / stats.inputBytes
    TierMetrics(
      tier,
      stats.inputBytes,
      stats.outputBytes,
      stats.ratio,
      perByte(encodeNanos),
      perByte(decodeNanos),
      stats.blocks
    )

/** Compresses data under the policy of a tier and reports the metrics. */
final class TierCodec private (policy: TierPolicy, codecs: Map[Tier, AceCodec]) extends AutoCloseable:

  /** The compressed bytes and the metrics of the run (decode time is measured too). */
  def compress(tier: Tier, data: Array[Byte]): Either[AceError, TieredResult] =
    val codec = codecs(tier)
    for
      timedEncode <- timed(codec.compressWithStats(data))
      timedDecode <- timed(codec.decompress(timedEncode._1.bytes))
      _ <- Either.cond(
        java.util.Arrays.equals(timedDecode._1, data),
        (),
        AceError.Internal("a tier round trip did not reproduce the input")
      )
    yield TieredResult(
      timedEncode._1.bytes,
      TierMetrics.fromStats(tier, timedEncode._1.stats, timedEncode._2, timedDecode._2)
    )

  def decompress(data: Array[Byte]): Either[AceError, Array[Byte]] = codecs(Tier.Warm).decompress(data)

  /** Metrics of every tier for the same sample, for comparing the tiers. */
  def measureAll(sample: Array[Byte]): Either[AceError, List[TierMetrics]] =
    Tier.values.toList.foldLeft[Either[AceError, List[TierMetrics]]](Right(Nil)) { (acc, tier) =>
      for
        done <- acc
        result <- compress(tier, sample)
      yield done :+ result.metrics
    }

  def config(tier: Tier): AceConfig = policy.configFor(tier)

  override def close(): Unit = codecs.values.foreach(_.close())

  private def timed[A](body: => Either[AceError, A]): Either[AceError, (A, Long)] =
    val start = System.nanoTime()
    body.map(value => (value, System.nanoTime() - start))

/** Compressed bytes with the metrics of the run. */
final case class TieredResult(bytes: Array[Byte], metrics: TierMetrics)

object TierCodec:
  /**
   * Opens one codec per tier. `dictionaries` are the external dictionaries the codecs resolve
   * when they read files written with [[DictionaryScope.External]].
   */
  def open(
      policy: TierPolicy = TierPolicy.Default,
      dictionaries: Seq[AceDictionary] = Nil
  ): Either[AceError, TierCodec] =
    Tier.values.toList.foldLeft[Either[AceError, Map[Tier, AceCodec]]](Right(Map.empty)) { (acc, tier) =>
      acc match
        case Left(error) => Left(error)
        case Right(opened) =>
          Ace.open(policy.configFor(tier), DecodeLimits.Default, dictionaries) match
            case Right(codec) => Right(opened + (tier -> codec))
            case Left(error) =>
              opened.values.foreach(_.close())
              Left(error)
    }.map(new TierCodec(policy, _))
