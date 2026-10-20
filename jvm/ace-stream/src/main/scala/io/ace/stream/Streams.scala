package io.ace.stream

import io.ace.model.*
import io.ace.sdk.*

import java.io.{ByteArrayOutputStream, InputStream, OutputStream}
import scala.util.control.NonFatal

/** Stream-shaped entry points. Memory use of `compress` and `decompress` is bounded by the block size. */
object AceStreams:

  /** Reads exactly `size` bytes from `in` and writes the ACE file to `out`. */
  def compress(codec: AceCodec, size: Long, in: InputStream, out: OutputStream): Either[AceError, CompressionStats] =
    try Right(io.ace.sdk.Convert.stats(codec.javaEngine.compressStream(size, in, out)))
    catch case e: Throwable => io.ace.sdk.Convert.failure(e)

  /** Decodes an ACE file read from `in` into `out`, block by block. */
  def decompress(codec: AceCodec, in: InputStream, out: OutputStream): Either[AceError, Unit] =
    try Right(codec.javaEngine.decompressStream(in, out))
    catch case e: Throwable => io.ace.sdk.Convert.failure(e)

  /** The blocks of an indexed file, in order, as an iterator. The first failure ends it with an exception. */
  def blocks(range: AceRange): Iterator[Array[Byte]] =
    new Iterator[Array[Byte]]:
      private var position = 0L
      override def hasNext: Boolean = position < range.blockCount
      override def next(): Array[Byte] =
        if !hasNext then throw new NoSuchElementException("no more blocks")
        val index = position
        position += 1
        range.readBlock(index) match
          case Right(bytes) => bytes
          case Left(error)  => throw AceStreamException(error)

/** Raised by iterator and stream adapters, which cannot return an `Either`. */
final class AceStreamException(val error: AceError) extends java.io.IOException(error.message)

/** Reads the original bytes of an indexed ACE file as an `InputStream`, one block at a time. */
final class AceInputStream(range: AceRange) extends InputStream:
  private var block: Array[Byte] = Array.emptyByteArray
  private var offset = 0
  private var nextBlock = 0L

  private def fill(): Boolean =
    while offset >= block.length && nextBlock < range.blockCount do
      range.readBlock(nextBlock) match
        case Right(bytes) => block = bytes; offset = 0; nextBlock += 1
        case Left(error)  => throw AceStreamException(error)
    offset < block.length

  override def read(): Int =
    if fill() then
      val b = block(offset) & 0xff
      offset += 1
      b
    else -1

  override def read(buffer: Array[Byte], off: Int, len: Int): Int =
    java.util.Objects.checkFromIndexSize(off, len, buffer.length)
    if len == 0 then 0
    else if !fill() then -1
    else
      val n = math.min(len, block.length - offset)
      System.arraycopy(block, offset, buffer, off, n)
      offset += n
      n

  override def available(): Int = math.max(0, block.length - offset)
  override def close(): Unit = range.close()

/** Collects what is written and writes one ACE file to `target` when it is closed. Memory use is the input size. */
final class AceOutputStream(codec: AceCodec, target: OutputStream) extends OutputStream:
  private val buffer = new ByteArrayOutputStream()
  private var closed = false
  private var result: Option[CompressionStats] = None

  override def write(b: Int): Unit = ensureOpen(buffer.write(b))
  override def write(b: Array[Byte], off: Int, len: Int): Unit = ensureOpen(buffer.write(b, off, len))

  private def ensureOpen(f: => Unit): Unit =
    if closed then throw new java.io.IOException("stream closed")
    f

  /** The statistics of the compression, available after `close()`. */
  def stats: Option[CompressionStats] = result

  override def close(): Unit =
    if !closed then
      closed = true
      codec.compressWithStats(buffer.toByteArray) match
        case Right(r) => target.write(r.bytes); target.flush(); result = Some(r.stats)
        case Left(e)  => throw AceStreamException(e)
