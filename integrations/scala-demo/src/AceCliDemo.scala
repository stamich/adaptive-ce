import scala.sys.process.*

/**
 * Minimal Scala demonstration of the stable ACE CLI boundary.
 *
 * This is deliberately not a native JVM binding. Production FFM/Panama integration remains a
 * later roadmap item so ACE 0.2.1-buildfix1 can keep Format 1.1 and the Rust execution core independent.
 */
object AceCliDemo:
  /**
   * Executes `ace verify` for one archive.
   *
   * @param aceExecutable path or command name of the ACE CLI executable
   * @param archive path to the ACE archive
   * @return process exit status
   */
  def verify(aceExecutable: String, archive: String): Int =
    Seq(aceExecutable, "verify", archive).!

  /**
   * Entry point for the optional Scala CLI integration demonstration.
   *
   * @param args `<ace-executable> <archive.ace>`
   */
  def main(args: Array[String]): Unit =
    require(args.length == 2, "usage: AceCliDemo <ace-executable> <archive.ace>")
    sys.exit(verify(args(0), args(1)))
