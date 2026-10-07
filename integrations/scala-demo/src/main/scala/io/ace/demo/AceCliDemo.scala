package io.ace.demo

import java.nio.file.Path
import scala.sys.process.Process

/**
 * Minimal Scala integration example for ACE 0.4.x (CLI process boundary).
 *
 * The milestone keeps Scala outside the native compression core and demonstrates only the stable
 * command-line boundary. Panama/FFM wrappers are intentionally deferred to a later milestone.
 */
object AceCliDemo {
  /**
   * Compresses one source file by invoking the ACE command-line executable.
   *
   * @param aceExecutable path to the built `ace` executable
   * @param input source file
   * @param output destination ACE file
   * @return process exit code
   */
  def compress(aceExecutable: Path, input: Path, output: Path): Int =
    Process(Seq(aceExecutable.toString, "compress", input.toString, output.toString)).!

  /**
   * Runs the minimal Scala process-integration demonstration.
   *
   * @param args executable, input and output paths in that order
   */
  def main(args: Array[String]): Unit = {
    require(args.length == 3, "usage: AceCliDemo ACE_EXECUTABLE INPUT OUTPUT")
    val exitCode = compress(Path.of(args(0)), Path.of(args(1)), Path.of(args(2)))
    require(exitCode == 0, s"ACE process failed with exit code $exitCode")
  }
}
