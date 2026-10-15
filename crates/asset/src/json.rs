//! A minimal, allocation-light JSON reader sufficient for glTF documents.
//!
//! It is deliberately not a general-purpose JSON library: no schemas, no
//! validation beyond well-formedness, no comments, no duplicate-key detection.
//! glTF requires all of that, which is why this covers exactly what the format
//! allows and rejects the rest.
//!
//! Objects are stored as a flat vector of `(key, value)` pairs because glTF
//! objects have few members. Lookup is linear, but for a document with a few
//! thousand small objects that is faster than building a hash map per object.

use std::collections::HashMap;

/// A parsed JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

/// A JSON error with the byte offset where it was detected.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonError {
    pub message: String,
    pub offset: usize,
}

impl JsonError {
    pub fn new(message: impl Into<String>, offset: usize) -> Self {
        Self {
            message: message.into(),
            offset,
        }
    }
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON error at byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for JsonError {}

impl JsonValue {
    /// Reads a member from an object, returning `None` if this is not an object
    /// or the key is absent.
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Object member lookup with a `usize` default for optional numeric fields.
    pub fn get_usize(&self, key: &str, default: usize) -> usize {
        self.get(key).and_then(JsonValue::as_usize).unwrap_or(default)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(JsonValue::as_str)
    }

    pub fn get_array(&self, key: &str) -> Option<&[JsonValue]> {
        self.get(key).and_then(JsonValue::as_array)
    }

    pub fn get_object(&self, key: &str) -> Option<&JsonValue> {
        self.get(key).filter(|v| matches!(v, JsonValue::Object(_)))
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsonValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            JsonValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        self.as_f64().map(|n| n as f32)
    }

    pub fn as_usize(&self) -> Option<usize> {
        self.as_f64().map(|n| n as usize)
    }

    pub fn as_u32(&self) -> Option<u32> {
        self.as_f64().map(|n| n as u32)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JsonValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            JsonValue::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Reads an array of numbers, ignoring anything that is not a number.
    pub fn number_array(&self) -> Vec<f64> {
        match self {
            JsonValue::Array(items) => items.iter().filter_map(JsonValue::as_f64).collect(),
            _ => Vec::new(),
        }
    }

    pub fn object_pairs(&self) -> Option<&[(String, JsonValue)]> {
        match self {
            JsonValue::Object(p) => Some(p),
            _ => None,
        }
    }

    /// Object members as a map, for the rare case where order is irrelevant.
    pub fn to_map(&self) -> HashMap<String, JsonValue> {
        match self {
            JsonValue::Object(pairs) => pairs.iter().cloned().collect(),
            _ => HashMap::new(),
        }
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            bytes: input.as_bytes(),
            pos: 0,
        }
    }

    fn err<T>(&self, message: impl Into<String>) -> Result<T, JsonError> {
        Err(JsonError::new(message, self.pos))
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek();
        if b.is_some() {
            self.pos += 1;
        }
        b
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r')) {
            self.pos += 1;
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, JsonError> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => Ok(JsonValue::String(self.parse_string()?)),
            Some(b't') => self.parse_literal("true", JsonValue::Bool(true)),
            Some(b'f') => self.parse_literal("false", JsonValue::Bool(false)),
            Some(b'n') => self.parse_literal("null", JsonValue::Null),
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            Some(_) => self.err("unexpected character"),
            None => self.err("unexpected end of input"),
        }
    }

    fn parse_literal(&mut self, literal: &str, value: JsonValue) -> Result<JsonValue, JsonError> {
        if self.bytes[self.pos..].starts_with(literal.as_bytes()) {
            self.pos += literal.len();
            Ok(value)
        } else {
            self.err(format!("expected '{}'", literal))
        }
    }

    fn parse_number(&mut self) -> Result<JsonValue, JsonError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while matches!(self.peek(), Some(b'0'..=b'9') | Some(b'.') | Some(b'e') | Some(b'E') | Some(b'+') | Some(b'-')) {
            self.pos += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| JsonError::new("invalid UTF-8 in number", start))?;
        text.parse::<f64>()
            .map(JsonValue::Number)
            .map_err(|_| JsonError::new(format!("invalid number '{}'", text), start))
    }

    fn parse_string(&mut self) -> Result<String, JsonError> {
        // Consumes the opening quote.
        if self.bump() != Some(b'"') {
            return self.err("expected '\"'");
        }

        let mut out = String::new();
        loop {
            match self.bump() {
                None => return self.err("unterminated string"),
                Some(b'"') => return Ok(out),
                Some(b'\\') => match self.bump() {
                    Some(b'"') => out.push('"'),
                    Some(b'\\') => out.push('\\'),
                    Some(b'/') => out.push('/'),
                    Some(b'b') => out.push('\u{0008}'),
                    Some(b'f') => out.push('\u{000C}'),
                    Some(b'n') => out.push('\n'),
                    Some(b'r') => out.push('\r'),
                    Some(b't') => out.push('\t'),
                    Some(b'u') => {
                        let hex = self
                            .bytes
                            .get(self.pos..self.pos + 4)
                            .and_then(|s| std::str::from_utf8(s).ok())
                            .ok_or_else(|| JsonError::new("truncated \\u escape", self.pos))?;
                        let code = u32::from_str_radix(hex, 16)
                            .map_err(|_| JsonError::new("invalid \\u escape", self.pos))?;
                        self.pos += 4;
                        // Handle surrogate pairs.
                        if (0xD800..0xDC00).contains(&code) {
                            if self.bytes[self.pos..].starts_with(b"\\u") {
                                let low_hex = std::str::from_utf8(&self.bytes[self.pos + 2..self.pos + 6])
                                    .ok()
                                    .and_then(|s| u32::from_str_radix(s, 16).ok());
                                if let Some(low) = low_hex {
                                    self.pos += 6;
                                    let c = 0x10000
                                        + ((code - 0xD800) << 10)
                                        + (low - 0xDC00);
                                    if let Some(ch) = char::from_u32(c) {
                                        out.push(ch);
                                        continue;
                                    }
                                }
                            }
                            out.push('\u{FFFD}');
                        } else if let Some(ch) = char::from_u32(code) {
                            out.push(ch);
                        } else {
                            out.push('\u{FFFD}');
                        }
                    }
                    _ => return self.err("invalid escape sequence"),
                },
                Some(b) if b < 0x20 => return self.err("control character in string"),
                Some(b) => {
                    // Copy this byte plus any UTF-8 continuation bytes. `bump`
                    // already advanced past the first byte, so it lives at
                    // `pos - 1`; reading from `self.pos` would overshoot by one
                    // byte and swallow the following character.
                    let start = self.pos - 1;
                    let len = utf8_len(b);
                    let end = (start + len).min(self.bytes.len());
                    if let Ok(s) = std::str::from_utf8(&self.bytes[start..end]) {
                        out.push_str(s);
                    }
                    self.pos = end;
                }
            }
        }
    }

    fn parse_array(&mut self) -> Result<JsonValue, JsonError> {
        self.pos += 1; // '['
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonValue::Array(items));
        }
        loop {
            let value = self.parse_value()?;
            items.push(value);
            self.skip_whitespace();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => return Ok(JsonValue::Array(items)),
                _ => return self.err("expected ',' or ']'"),
            }
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, JsonError> {
        self.pos += 1; // '{'
        let mut pairs: Vec<(String, JsonValue)> = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(JsonValue::Object(pairs));
        }
        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            self.skip_whitespace();
            if self.bump() != Some(b':') {
                return self.err("expected ':'");
            }
            let value = self.parse_value()?;
            pairs.push((key, value));
            self.skip_whitespace();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => return Ok(JsonValue::Object(pairs)),
                _ => return self.err("expected ',' or '}'"),
            }
        }
    }
}

fn utf8_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first >> 5 == 0b110 {
        2
    } else if first >> 4 == 0b1110 {
        3
    } else if first >> 3 == 0b11110 {
        4
    } else {
        1
    }
}

/// Parses a JSON document, rejecting trailing content.
pub fn parse_json(input: &str) -> Result<JsonValue, JsonError> {
    let mut parser = Parser::new(input);
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.pos != parser.bytes.len() {
        return Err(JsonError::new("trailing content after JSON value", parser.pos));
    }
    Ok(value)
}
