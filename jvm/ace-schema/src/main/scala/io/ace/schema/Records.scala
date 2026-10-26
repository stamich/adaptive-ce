package io.ace.schema

import io.ace.model.{AceError, Column, Schema}

import java.nio.{ByteBuffer, ByteOrder}
import scala.compiletime.{constValueTuple, summonAll}
import scala.deriving.Mirror

/**
 * How one field of a record is laid out: its [[Column]] and how it is written to and read from
 * little-endian bytes, the byte order ACE columns use.
 */
trait FieldCodec[A]:
  /** The column this field occupies in the schema. */
  def column: Column

  /** Writes the field at the buffer's position. */
  def write(a: A, buffer: ByteBuffer): Unit

  /** Reads the field at the buffer's position. */
  def read(buffer: ByteBuffer): A

object FieldCodec:
  private def of[A](c: Column)(w: (A, ByteBuffer) => Unit)(r: ByteBuffer => A): FieldCodec[A] =
    new FieldCodec[A]:
      val column = c
      def write(a: A, buffer: ByteBuffer): Unit = w(a, buffer)
      def read(buffer: ByteBuffer): A = r(buffer)

  given FieldCodec[Byte] = of[Byte](Column.signed(1))((a, b) => { b.put(a); () })(_.get())
  given FieldCodec[Short] = of[Short](Column.signed(2))((a, b) => { b.putShort(a); () })(_.getShort())
  given FieldCodec[Int] = of[Int](Column.signed(4))((a, b) => { b.putInt(a); () })(_.getInt())
  given FieldCodec[Long] = of[Long](Column.signed(8))((a, b) => { b.putLong(a); () })(_.getLong())
  given FieldCodec[Float] = of[Float](Column.float(4))((a, b) => { b.putFloat(a); () })(_.getFloat())
  given FieldCodec[Double] = of[Double](Column.float(8))((a, b) => { b.putDouble(a); () })(_.getDouble())
  /** A `Char` is a UTF-16 code unit: an unsigned 16-bit column. */
  given FieldCodec[Char] = of[Char](Column.unsigned(2))((a, b) => { b.putChar(a); () })(_.getChar())
  /** A `Boolean` is one byte, 0 or 1; a column of flags. */
  given FieldCodec[Boolean] = of[Boolean](Column.bytes(1))((a, b) => { b.put(if a then 1.toByte else 0.toByte); () })(_.get() != 0)

/**
 * Turns case-class records into the fixed-size byte records that ACE stores column by column, and
 * back, and knows the [[Schema]] that describes them. The schema and the byte layout come from the
 * same field list, so they cannot disagree.
 *
 * {{{
 * final case class Trade(time: ULong, price: Double, quantity: UInt, side: Boolean)
 *   derives RecordCodec
 *
 * val codec = summon[RecordCodec[Trade]]
 * val config = AceConfig(schema = Some(codec.schema))
 * val bytes = codec.encode(trades)               // 21 bytes per record
 * }}}
 *
 * The fields of the case class, in declaration order, become the columns: `Byte`, `Short`, `Int`
 * and `Long` are signed integers, [[UByte]], [[UShort]], [[UInt]], [[ULong]] and `Char` are
 * unsigned, `Float` and `Double` are floats, `Boolean` is a one-byte flag. Other field types need
 * a given [[FieldCodec]].
 */
trait RecordCodec[T]:
  /** The layout of one record. */
  def schema: Schema

  /** Bytes per record. */
  final def recordSize: Int = schema.recordSize

  /** Appends the bytes of `record` to `buffer` (little-endian). */
  def write(record: T, buffer: ByteBuffer): Unit

  /** Reads one record from the buffer's position. */
  def read(buffer: ByteBuffer): T

  /** The bytes of all `records`, one record after the other. */
  final def encode(records: Iterable[T]): Array[Byte] =
    val buffer = ByteBuffer.allocate(Math.multiplyExact(records.size, recordSize)).order(ByteOrder.LITTLE_ENDIAN)
    records.foreach(write(_, buffer))
    buffer.array()

  /** The records in `bytes`, which must hold a whole number of them. */
  final def decode(bytes: Array[Byte]): Either[AceError, Vector[T]] =
    if recordSize == 0 || bytes.length % recordSize != 0 then
      Left(AceError.InvalidArgument(s"${bytes.length} bytes are not a whole number of $recordSize-byte records"))
    else
      val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
      Right(Vector.fill(bytes.length / recordSize)(read(buffer)))

object RecordCodec:
  /** Derives the codec of a case class: `final case class Row(...) derives RecordCodec`. */
  inline def derived[T](using m: Mirror.ProductOf[T]): RecordCodec[T] =
    val fields = summonAll[Tuple.Map[m.MirroredElemTypes, FieldCodec]].toList.asInstanceOf[List[FieldCodec[Any]]]
    new ProductRecordCodec[T](fields, values => m.fromProduct(Tuple.fromArray(values)))

  /** The codec of `T` without a `derives` clause. */
  inline def of[T](using m: Mirror.ProductOf[T]): RecordCodec[T] = derived[T]

/** The codec of a product type; created by [[RecordCodec.derived]]. */
final class ProductRecordCodec[T](fields: List[FieldCodec[Any]], build: Array[Any] => T) extends RecordCodec[T]:
  val schema: Schema = Schema(fields.map(_.column))

  def write(record: T, buffer: ByteBuffer): Unit =
    val product = record.asInstanceOf[Product]
    var i = 0
    while i < fields.length do
      fields(i).write(product.productElement(i), buffer)
      i += 1

  def read(buffer: ByteBuffer): T =
    val values = new Array[Any](fields.length)
    var i = 0
    while i < fields.length do
      values(i) = fields(i).read(buffer)
      i += 1
    build(values)
