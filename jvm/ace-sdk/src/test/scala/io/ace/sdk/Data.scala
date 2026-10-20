package io.ace.sdk

import java.nio.{ByteBuffer, ByteOrder}

/** Deterministic inputs for the SDK tests. */
object Data:
  def counters(count: Int): Array[Byte] =
    val b = ByteBuffer.allocate(count * 4).order(ByteOrder.LITTLE_ENDIAN)
    var i = 0
    while i < count do
      b.putInt(i / 3)
      i += 1
    b.array()

  def text(length: Int): Array[Byte] =
    val words = "adaptive compression engine lossless deterministic random access ".getBytes
    Array.tabulate(length)(i => words((i * 7 + i / words.length) % words.length))

  def random(length: Int, seed: Long): Array[Byte] =
    val out = new Array[Byte](length)
    new java.util.Random(seed).nextBytes(out)
    out
