package io.ace;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * A small JSON reader for the documents of the C ABI (Stats, planner report) and the golden file.
 * Objects become {@code LinkedHashMap<String, Object>}, arrays {@code List<Object>}, integers
 * {@code Long}, other numbers {@code Double}.
 */
final class Json {
    private final String text;
    private int position;

    private Json(String text) {
        this.text = text;
    }

    static Object parse(String text) {
        Json parser = new Json(text);
        Object value = parser.value();
        parser.skipWhitespace();
        if (parser.position != text.length()) {
            throw parser.error("unexpected trailing text");
        }
        return value;
    }

    @SuppressWarnings("unchecked")
    static Map<String, Object> parseObject(String text) {
        return (Map<String, Object>) parse(text);
    }

    private IllegalArgumentException error(String message) {
        return new IllegalArgumentException("JSON: " + message + " at offset " + position);
    }

    private void skipWhitespace() {
        while (position < text.length() && Character.isWhitespace(text.charAt(position))) {
            position++;
        }
    }

    private char peek() {
        skipWhitespace();
        if (position >= text.length()) {
            throw error("unexpected end");
        }
        return text.charAt(position);
    }

    private void expect(char c) {
        if (peek() != c) {
            throw error("expected '" + c + "'");
        }
        position++;
    }

    private Object value() {
        char c = peek();
        if (c == '{') {
            return object();
        } else if (c == '[') {
            return array();
        } else if (c == '"') {
            return string();
        } else if (text.startsWith("true", position)) {
            position += 4;
            return Boolean.TRUE;
        } else if (text.startsWith("false", position)) {
            position += 5;
            return Boolean.FALSE;
        } else if (text.startsWith("null", position)) {
            position += 4;
            return null;
        }
        return number();
    }

    private Map<String, Object> object() {
        Map<String, Object> map = new LinkedHashMap<>();
        expect('{');
        if (peek() == '}') {
            position++;
            return map;
        }
        while (true) {
            String key = string();
            expect(':');
            map.put(key, value());
            char c = peek();
            position++;
            if (c == '}') {
                return map;
            }
            if (c != ',') {
                throw error("expected ',' or '}'");
            }
        }
    }

    private List<Object> array() {
        List<Object> list = new ArrayList<>();
        expect('[');
        if (peek() == ']') {
            position++;
            return list;
        }
        while (true) {
            list.add(value());
            char c = peek();
            position++;
            if (c == ']') {
                return list;
            }
            if (c != ',') {
                throw error("expected ',' or ']'");
            }
        }
    }

    private String string() {
        expect('"');
        StringBuilder out = new StringBuilder();
        while (position < text.length()) {
            char c = text.charAt(position++);
            if (c == '"') {
                return out.toString();
            }
            if (c != '\\') {
                out.append(c);
                continue;
            }
            if (position >= text.length()) {
                break;
            }
            char escape = text.charAt(position++);
            switch (escape) {
                case '"', '\\', '/' -> out.append(escape);
                case 'n' -> out.append('\n');
                case 'r' -> out.append('\r');
                case 't' -> out.append('\t');
                case 'b' -> out.append('\b');
                case 'f' -> out.append('\f');
                case 'u' -> {
                    if (position + 4 > text.length()) {
                        throw error("bad unicode escape");
                    }
                    out.append((char) Integer.parseInt(text.substring(position, position + 4), 16));
                    position += 4;
                }
                default -> throw error("bad escape");
            }
        }
        throw error("unterminated string");
    }

    private Object number() {
        int start = position;
        while (position < text.length() && "+-0123456789.eE".indexOf(text.charAt(position)) >= 0) {
            position++;
        }
        String token = text.substring(start, position);
        if (token.isEmpty()) {
            throw error("unexpected character");
        }
        try {
            if (token.indexOf('.') < 0 && token.indexOf('e') < 0 && token.indexOf('E') < 0) {
                return Long.parseLong(token);
            }
            return Double.parseDouble(token);
        } catch (NumberFormatException e) {
            throw error("bad number '" + token + "'");
        }
    }
}
