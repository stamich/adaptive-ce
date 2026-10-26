package io.ace.schema

import java.nio.ByteBuffer

/*
 * Scala has no unsigned integers. These opaque types mark a record field as unsigned, so the
 * schema inferred for a case class says `Unsigned` (counters, ids, timestamps) rather than
 * `Signed`. They cost nothing at run time: each is its signed primitive.
 */

/** An unsigned 8-bit integer (0 to 255). */
opaque type UByte = Byte

object UByte:
  /** @throws IllegalArgumentException when `value` is outside 0 to 255 */
  def apply(value: Int): UByte =
    require(value >= 0 && value <= 0xff, s"not an unsigned byte: $value")
    value.toByte

  extension (u: UByte) def toInt: Int = u & 0xff

  given FieldCodec[UByte] with
    val column = io.ace.model.Column.unsigned(1)
    def write(a: UByte, buffer: ByteBuffer): Unit = { buffer.put(a); () }
    def read(buffer: ByteBuffer): UByte = buffer.get()

/** An unsigned 16-bit integer (0 to 65 535). */
opaque type UShort = Short

object UShort:
  /** @throws IllegalArgumentException when `value` is outside 0 to 65 535 */
  def apply(value: Int): UShort =
    require(value >= 0 && value <= 0xffff, s"not an unsigned short: $value")
    value.toShort

  extension (u: UShort) def toInt: Int = u & 0xffff

  given FieldCodec[UShort] with
    val column = io.ace.model.Column.unsigned(2)
    def write(a: UShort, buffer: ByteBuffer): Unit = { buffer.putShort(a); () }
    def read(buffer: ByteBuffer): UShort = buffer.getShort()

/** An unsigned 32-bit integer (0 to 4 294 967 295). */
opaque type UInt = Int

object UInt:
  /** @throws IllegalArgumentException when `value` is outside 0 to 4 294 967 295 */
  def apply(value: Long): UInt =
    require(value >= 0 && value <= 0xffffffffL, s"not an unsigned int: $value")
    value.toInt

  extension (u: UInt) def toLong: Long = u & 0xffffffffL

  given FieldCodec[UInt] with
    val column = io.ace.model.Column.unsigned(4)
    def write(a: UInt, buffer: ByteBuffer): Unit = { buffer.putInt(a); () }
    def read(buffer: ByteBuffer): UInt = buffer.getInt()

/** An unsigned 64-bit integer, held in the bits of a `Long`. */
opaque type ULong = Long

object ULong:
  /** The unsigned number with the same 64 bits as `bits` (negative values are above 2^63). */
  def fromBits(bits: Long): ULong = bits

  /** @throws IllegalArgumentException when `value` is negative */
  def apply(value: Long): ULong =
    require(value >= 0, s"not an unsigned long: $value")
    value

  extension (u: ULong)
    /** The 64 bits as a `Long`; use [[toBigInt]] or [[show]] for the unsigned value. */
    def bits: Long = u

    def toBigInt: BigInt = BigInt(java.lang.Long.toUnsignedString(u))

    def show: String = java.lang.Long.toUnsignedString(u)

  given FieldCodec[ULong] with
    val column = io.ace.model.Column.unsigned(8)
    def write(a: ULong, buffer: ByteBuffer): Unit = { buffer.putLong(a); () }
    def read(buffer: ByteBuffer): ULong = buffer.getLong()
