package io.ace.demo;

import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class DemoTest {
    @Test
    void theJavaDemoRuns() {
        String output = JavaDemo.run();
        assertTrue(output.contains("round trip verified"), output);
        assertTrue(output.contains("range   64 bytes"), output);
    }

    @Test
    void theScalaDemoRuns() {
        String output = ScalaDemo.run().fold(error -> error.message(), text -> text);
        assertTrue(output.contains("Hot") && output.contains("Cold"), output);
    }
}
