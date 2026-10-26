package io.ace.schema

import io.ace.model.*
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

import java.util.HexFormat

final case class Trade(time: ULong, price: Double, quantity: UInt, side: Boolean) derives RecordCodec

final case class Reading(sensor: Short, celsius: Float, delta: Int, count: UShort, flags: UByte, mark: Char, id: Long, small: Byte)

class RecordCodecTest:

  private def hex(bytes: Array[Byte]): String = HexFormat.of().formatHex(bytes)

  private val codec = summon[RecordCodec[Trade]]

  @Test def theSchemaFollowsTheFieldsInOrder(): Unit =
    assertEquals(
      Schema.of(Column.unsigned(8), Column.float(8), Column.unsigned(4), Column.bytes(1)),
      codec.schema
    )
    assertEquals(21, codec.recordSize)
    assertEquals(Nil, codec.schema.validate)

  @Test def everySupportedFieldTypeMapsToItsColumn(): Unit =
    val schema = RecordCodec.of[Reading].schema
    assertEquals(
      List(
        Column.signed(2), Column.float(4), Column.signed(4), Column.unsigned(2),
        Column.unsigned(1), Column.unsigned(2), Column.signed(8), Column.signed(1)
      ),
      schema.columns
    )
    assertEquals(Nil, schema.validate)

  /** The byte layout is part of the contract: little-endian fields, records back to back. */
  @Test def goldenBytesOfARecord(): Unit =
    val bytes = codec.encode(List(Trade(ULong(1L), 2.0, UInt(3L), true)))
    assertEquals("0100000000000000" + "0000000000000040" + "03000000" + "01", hex(bytes))

  @Test def unsignedValuesKeepTheirBits(): Unit =
    val max = Trade(ULong.fromBits(-1L), 0.0, UInt(0xffffffffL), false)
    val bytes = codec.encode(List(max))
    assertEquals("ffffffffffffffff" + "0000000000000000" + "ffffffff" + "00", hex(bytes))
    val back = codec.decode(bytes).toOption.get.head
    assertEquals("18446744073709551615", back.time.show)
    assertEquals(0xffffffffL, back.quantity.toLong)
    assertEquals(BigInt("18446744073709551615"), back.time.toBigInt)

  @Test def recordsRoundTrip(): Unit =
    val trades = (0 until 1000).map(i => Trade(ULong(1_700_000_000L + i * 5L), 100 + (i % 50) * 0.25, UInt(i % 300L), i % 2 == 0)).toVector
    assertEquals(Right(trades), codec.decode(codec.encode(trades)))
    assertEquals(Right(Vector.empty), codec.decode(Array.emptyByteArray))

  @Test def aPartialRecordIsRejected(): Unit =
    codec.decode(new Array[Byte](22)) match
      case Left(_: AceError.InvalidArgument) => ()
      case other                             => fail(s"unexpected $other")

  @Test def unsignedConstructorsCheckTheirRange(): Unit =
    assertThrows(classOf[IllegalArgumentException], () => UByte(256))
    assertThrows(classOf[IllegalArgumentException], () => UShort(-1))
    assertThrows(classOf[IllegalArgumentException], () => UInt(1L << 32))
    assertThrows(classOf[IllegalArgumentException], () => ULong(-1L))
    assertEquals(255, UByte(255).toInt)
    assertEquals(65535, UShort(65535).toInt)
