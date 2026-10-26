package io.ace.schema

import io.ace.model.*
import io.ace.sdk.{Ace, AceCodec}
import io.ace.tier.{TierCodec, TierPolicy}

/**
 * The Format 1.6 settings that fit each storage tier, as a [[TierPolicy]] for `ace-tier`.
 *
 * What the tiers have in common is the data: the same `schema` and `dictionary` describe it
 * whether it is hot or cold. What differs is how it is read and how long it lives:
 *
 *  - '''Hot''' data is read often in small ranges. Fast encoding, 64 KiB blocks, a random-access
 *    index. An external dictionary: small records gain the most from it, and the readers are the
 *    same process that holds the [[DictionaryStore]], so the file need not carry it.
 *  - '''Warm''' data is the library default (balanced, 256 KiB) with the same external dictionary.
 *  - '''Cold''' data is written once and may outlive the store. Dense encoding, 1 MiB blocks,
 *    segmentation (an archive is often a mix of text, counters and noise), and the dictionary
 *    '''embedded''', so the file is self-contained.
 *
 * Every Format 1.6 feature is chosen by the block, so a schema or dictionary that does not fit
 * the data costs time but never space.
 */
object StorageProfiles:

  /** Segments of a cold archive: windows of 16 KiB, segments from 64 KiB. */
  val ColdSegmentation: Segmentation = Segmentation()

  /**
   * The policy for data described by `schema` and compressed with `dictionary`; both are optional.
   * Starts from [[TierPolicy.Default]], so block sizes, profiles and access hints are the ones
   * `ace-tier` already uses.
   */
  def tierPolicy(schema: Option[Schema] = None, dictionary: Option[AceDictionary] = None): TierPolicy =
    val base = TierPolicy.Default
    def shared(config: AceConfig, scope: DictionaryScope): AceConfig =
      config.copy(schema = schema, dictionary = dictionary.map(DictionaryUse(_, scope)))
    TierPolicy(
      hot = shared(base.hot, DictionaryScope.External),
      warm = shared(base.warm, DictionaryScope.External),
      cold = shared(base.cold, DictionaryScope.Embedded).copy(segmentation = Some(ColdSegmentation))
    )

  /** The policy for records of type `T`: the schema comes from its [[RecordCodec]]. */
  def forRecords[T](dictionary: Option[AceDictionary] = None)(using codec: RecordCodec[T]): TierPolicy =
    tierPolicy(Some(codec.schema), dictionary)

  /**
   * Opens a [[TierCodec]] for `policy` whose readers resolve the external dictionaries in `store`.
   * Every dictionary of the store is passed along, so files written with any of them can be read.
   */
  def openTiers(policy: TierPolicy, store: DictionaryStore): Either[AceError, TierCodec] =
    store.all.flatMap(dictionaries => TierCodec.open(policy, dictionaries))

  /** Opens one codec for `config` whose readers resolve the external dictionaries in `store`. */
  def open(config: AceConfig, store: DictionaryStore): Either[AceError, AceCodec] =
    store.all.flatMap(dictionaries => Ace.open(config, DecodeLimits.Default, dictionaries))
