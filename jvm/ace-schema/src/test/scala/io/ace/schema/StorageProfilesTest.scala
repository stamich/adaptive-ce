package io.ace.schema

import io.ace.model.*
import io.ace.sdk.Ace
import io.ace.tier.{Tier, TierCodec}
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

import java.nio.charset.StandardCharsets

class StorageProfilesTest:

  private def ok[A](r: Either[AceError, A]): A = r match
    case Right(a) => a
    case Left(e)  => fail(s"unexpected error: $e").asInstanceOf[A]

  private val dictionary = AceDictionary(0x2a2aL, "temperature_reading celsius plant-".getBytes)
  private val schema = Schema.of(Column.unsigned(8), Column.float(8))

  @Test def theTierSettingsDifferWhereTheTiersDo(): Unit =
    val policy = StorageProfiles.tierPolicy(Some(schema), Some(dictionary))
    assertEquals(Some(schema), policy.hot.schema)
    assertEquals(Some(schema), policy.warm.schema)
    assertEquals(Some(schema), policy.cold.schema)
    assertEquals(Some(DictionaryUse(dictionary, DictionaryScope.External)), policy.hot.dictionary)
    assertEquals(Some(DictionaryUse(dictionary, DictionaryScope.External)), policy.warm.dictionary)
    assertEquals(Some(DictionaryUse(dictionary, DictionaryScope.Embedded)), policy.cold.dictionary, "cold files are self-contained")
    assertEquals(None, policy.hot.segmentation)
    assertEquals(None, policy.warm.segmentation)
    assertEquals(Some(StorageProfiles.ColdSegmentation), policy.cold.segmentation)
    for tier <- Tier.values do assertEquals(Nil, policy.configFor(tier).validate, tier.toString)

  @Test def theBaseSettingsOfTierPolicyAreKept(): Unit =
    val policy = StorageProfiles.tierPolicy()
    val base = io.ace.tier.TierPolicy.Default
    assertEquals(base, policy.copy(cold = policy.cold.copy(segmentation = None)))
    assertEquals(None, policy.hot.schema)
    assertEquals(None, policy.cold.dictionary)

  @Test def aRecordTypeGivesItsSchemaToThePolicy(): Unit =
    val policy = StorageProfiles.forRecords[Trade]()
    assertEquals(Some(summon[RecordCodec[Trade]].schema), policy.hot.schema)

  /** Records go through the whole path: case class, derived schema, tier codec, store, readers. */
  @Test def tieredRecordsRoundTripWithAStoredDictionary(): Unit =
    val codec = summon[RecordCodec[Trade]]
    val trades = (0 until 20_000).map(i => Trade(ULong(1_700_000_000L + i * 5L), 100 + (i % 50) * 0.25, UInt(i % 3L), i % 2 == 0))
    val bytes = codec.encode(trades)
    val store = DictionaryStore.inMemory
    ok(store.put(dictionary))
    val policy = StorageProfiles.forRecords[Trade](Some(dictionary))
    val tiers = ok(StorageProfiles.openTiers(policy, store))
    try
      for tier <- Tier.values do
        val result = ok(tiers.compress(tier, bytes))
        assertEquals(bytes.length.toLong, result.metrics.inputBytes)
        assertEquals(Right(trades.toVector), codec.decode(ok(tiers.decompress(result.bytes))), tier.toString)
    finally tiers.close()

  @Test def aDerivedSchemaCompressesATableBetterThanRows(): Unit =
    val codec = summon[RecordCodec[Trade]]
    val trades = (0 until 20_000).map(i => Trade(ULong(1_700_000_000L + i * 5L), 100 + (i % 50) * 0.25, UInt(i % 3L), i % 2 == 0))
    val bytes = codec.encode(trades)
    val withSchema = ok(Ace.use(AceConfig(schema = Some(codec.schema)))(_.compressWithStats(bytes)))
    val rows = ok(Ace.use()(_.compress(bytes)))
    assertTrue(withSchema.stats.columnarBlocks > 0)
    assertTrue(withSchema.bytes.length < rows.length)

  @Test def openWithAStoreResolvesExternalDictionaries(): Unit =
    val store = DictionaryStore.inMemory
    ok(store.put(dictionary))
    val writer = AceConfig(blockSize = 4096, dictionary = Some(DictionaryUse(dictionary, DictionaryScope.External)))
    val text = ("temperature_reading celsius plant-3 " * 200).getBytes(StandardCharsets.UTF_8)
    val packed = ok(Ace.use(writer)(_.compress(text)))
    val reader = ok(StorageProfiles.open(AceConfig.Default, store))
    try assertArrayEquals(text, ok(reader.decompress(packed)))
    finally reader.close()
    ok(Ace.use()(plain => Right(assertTrue(plain.decompress(packed).left.exists(_.isInstanceOf[AceError.Unsupported])))))

  /** Both bindings call the same library with the same settings, so they write the same bytes. */
  @Test def theScalaSdkWritesTheBytesOfTheJavaSdk(): Unit =
    val codec = summon[RecordCodec[Trade]]
    val bytes = codec.encode((0 until 20_000).map(i => Trade(ULong(i.toLong), i * 0.5, UInt(i.toLong % 7), i % 2 == 0)))
    val javaConfig = io.ace.Config.builder()
      .schema(io.ace.Schema.of(io.ace.Column.unsigned(8), io.ace.Column.floating(8), io.ace.Column.unsigned(4), io.ace.Column.bytes(1)))
      .dictionary(io.ace.Dictionary.of(dictionary.id, dictionary.bytes))
      .segmentation(io.ace.Segmentation.defaults())
      .build()
    val expected = io.ace.Engine.create(javaConfig)
    try
      val scalaConfig = AceConfig(
        schema = Some(codec.schema),
        dictionary = Some(DictionaryUse(dictionary)),
        segmentation = Some(Segmentation())
      )
      assertArrayEquals(expected.compress(bytes), ok(Ace.use(scalaConfig)(_.compress(bytes))))
    finally expected.close()
