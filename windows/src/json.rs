//! JSON value model, serializer and parser.
//! Faithful Rust port of `Sources/JSONViewerCore/JSONValue.swift`.
//! Preserves object key ordering, raw number literals, comments,
//! single-quoted strings, trailing commas and line/column errors.

#[derive(Clone, Debug, PartialEq)]
pub struct JSONProperty {
    pub key: String,
    pub value: JSONValue,
}

impl JSONProperty {
    pub fn new(key: String, value: JSONValue) -> Self {
        Self { key, value }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum JSONValue {
    Null,
    Bool(bool),
    Number(f64, String),
    Str(String),
    Array(Vec<JSONValue>),
    Object(Vec<JSONProperty>),
}

impl JSONValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            JSONValue::Null => "null",
            JSONValue::Bool(_) => "boolean",
            JSONValue::Number(_, _) => "number",
            JSONValue::Str(_) => "string",
            JSONValue::Array(_) => "array",
            JSONValue::Object(_) => "object",
        }
    }

    pub fn is_container(&self) -> bool {
        matches!(self, JSONValue::Object(_) | JSONValue::Array(_))
    }

    pub fn child_count(&self) -> usize {
        match self {
            JSONValue::Object(p) => p.len(),
            JSONValue::Array(a) => a.len(),
            _ => 0,
        }
    }

    pub fn summary(&self) -> String {
        match self {
            JSONValue::Null => "null".to_string(),
            JSONValue::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
            JSONValue::Number(_, raw) => raw.clone(),
            JSONValue::Str(s) => format!("\"{}\"", s),
            JSONValue::Array(items) => format!("Array [{}]", items.len()),
            JSONValue::Object(pairs) => format!("Object {{{}}}", pairs.len()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct JSONFormatOptions {
    pub indent_spaces: i32,
    pub sort_keys: bool,
    pub escape_slashes: bool,
}

impl JSONFormatOptions {
    pub fn new(indent_spaces: i32, sort_keys: bool, escape_slashes: bool) -> Self {
        Self { indent_spaces, sort_keys, escape_slashes }
    }
}

impl Default for JSONFormatOptions {
    fn default() -> Self {
        Self { indent_spaces: 2, sort_keys: false, escape_slashes: false }
    }
}

impl JSONValue {
    pub fn format(&self, indent_spaces: i32, sort_keys: bool, escape_slashes: bool) -> String {
        self.format_with_options(&JSONFormatOptions::new(indent_spaces, sort_keys, escape_slashes))
    }

    pub fn format_with_options(&self, options: &JSONFormatOptions) -> String {
        let mut out = String::new();
        self.format_into(&mut out, 0, options);
        out
    }

    fn format_into(&self, out: &mut String, current_indent: usize, options: &JSONFormatOptions) {
        let indent: String = if options.indent_spaces < 0 {
            "\t".repeat(current_indent)
        } else {
            " ".repeat(current_indent)
        };
        let step: usize = if options.indent_spaces < 0 { 1 } else { options.indent_spaces as usize };
        let child_indent: String = if options.indent_spaces < 0 {
            "\t".repeat(current_indent + 1)
        } else {
            " ".repeat(current_indent + step)
        };
        match self {
            JSONValue::Null => out.push_str("null"),
            JSONValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            JSONValue::Number(_, raw) => out.push_str(raw),
            JSONValue::Str(s) => out.push_str(&escape_string(s, options.escape_slashes)),
            JSONValue::Array(items) => {
                if items.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push_str("[\n");
                for (idx, item) in items.iter().enumerate() {
                    out.push_str(&child_indent);
                    item.format_into(out, current_indent + step, options);
                    if idx + 1 < items.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&indent);
                out.push(']');
            }
            JSONValue::Object(pairs) => {
                if pairs.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push_str("{\n");
                let order: Vec<&JSONProperty>;
                let sorted: Vec<JSONProperty>;
                if options.sort_keys {
                    let mut v = pairs.clone();
                    v.sort_by(|a, b| a.key.cmp(&b.key));
                    sorted = v;
                    order = sorted.iter().collect();
                } else {
                    order = pairs.iter().collect();
                    sorted = Vec::new();
                    let _ = &sorted;
                }
                for (idx, pair) in order.iter().enumerate() {
                    out.push_str(&child_indent);
                    out.push_str(&escape_string(&pair.key, options.escape_slashes));
                    out.push_str(": ");
                    pair.value.format_into(out, current_indent + step, options);
                    if idx + 1 < order.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&indent);
                out.push('}');
            }
        }
    }

    pub fn minify(&self, escape_slashes: bool) -> String {
        let mut out = String::new();
        self.minify_into(&mut out, escape_slashes);
        out
    }

    fn minify_into(&self, out: &mut String, escape_slashes: bool) {
        match self {
            JSONValue::Null => out.push_str("null"),
            JSONValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            JSONValue::Number(_, raw) => out.push_str(raw),
            JSONValue::Str(s) => out.push_str(&escape_string(s, escape_slashes)),
            JSONValue::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    item.minify_into(out, escape_slashes);
                    if i + 1 < items.len() {
                        out.push(',');
                    }
                }
                out.push(']');
            }
            JSONValue::Object(pairs) => {
                out.push('{');
                for (i, pair) in pairs.iter().enumerate() {
                    out.push_str(&escape_string(&pair.key, escape_slashes));
                    out.push(':');
                    pair.value.minify_into(out, escape_slashes);
                    if i + 1 < pairs.len() {
                        out.push(',');
                    }
                }
                out.push('}');
            }
        }
    }

    pub fn stringify(&self, escape_slashes: bool) -> String {
        let min = self.minify(false);
        quote_and_escape_string(&min, escape_slashes)
    }

    pub fn to_python_object(&self, indent_spaces: usize) -> String {
        let mut out = String::new();
        self.python_into(&mut out, 0, indent_spaces);
        out
    }

    fn python_into(&self, out: &mut String, current_indent: usize, indent_spaces: usize) {
        let indent = " ".repeat(current_indent);
        let child_indent = " ".repeat(current_indent + indent_spaces);
        match self {
            JSONValue::Null => out.push_str("None"),
            JSONValue::Bool(b) => out.push_str(if *b { "True" } else { "False" }),
            JSONValue::Number(_, raw) => out.push_str(raw),
            JSONValue::Str(s) => out.push_str(&escape_python_string(s)),
            JSONValue::Array(items) => {
                if items.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    out.push_str(&child_indent);
                    item.python_into(out, current_indent + indent_spaces, indent_spaces);
                    if i + 1 < items.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&indent);
                out.push(']');
            }
            JSONValue::Object(pairs) => {
                if pairs.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push_str("{\n");
                for (i, pair) in pairs.iter().enumerate() {
                    out.push_str(&child_indent);
                    out.push_str(&escape_python_string(&pair.key));
                    out.push_str(": ");
                    pair.value.python_into(out, current_indent + indent_spaces, indent_spaces);
                    if i + 1 < pairs.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&indent);
                out.push('}');
            }
        }
    }
}

pub fn quote_and_escape_string(s: &str, escape_slashes: bool) -> String {
    escape_string(s, escape_slashes)
}

pub fn escape_string(s: &str, escape_slashes: bool) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '/' => {
                if escape_slashes {
                    out.push_str("\\/");
                } else {
                    out.push('/');
                }
            }
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c => {
                let v = c as u32;
                if v < 0x20 {
                    out.push_str(&format!("\\u{:04x}", v));
                } else {
                    out.push(c);
                }
            }
        }
    }
    out.push('"');
    out
}

fn escape_python_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        match ch {
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c => {
                let v = c as u32;
                if v < 0x20 {
                    out.push_str(&format!("\\x{:02x}", v));
                } else {
                    out.push(c);
                }
            }
        }
    }
    out.push('\'');
    out
}

/// Convert an escaped / stringified JSON payload back to formatted JSON.
/// Mirrors `JSONValue.unescapeStringifiedJSON` in Swift.
pub fn unescape_stringified_json(text: &str) -> String {
    let mut s = text.trim().to_string();
    if s.is_empty() {
        return s;
    }
    if s.len() >= 2 {
        let starts_dq = s.starts_with('"') && s.ends_with('"');
        let starts_sq = s.starts_with('\'') && s.ends_with('\'');
        if starts_dq || starts_sq {
            s = s[1..s.len() - 1].to_string();
        }
    }
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let next = match chars.next() {
                Some(n) => n,
                None => {
                    result.push(ch);
                    break;
                }
            };
            match next {
                '"' => result.push('"'),
                '\'' => result.push('\''),
                '\\' => result.push('\\'),
                '/' => result.push('/'),
                'b' => result.push('\u{08}'),
                'f' => result.push('\u{0C}'),
                'n' => result.push('\n'),
                'r' => result.push('\r'),
                't' => result.push('\t'),
                'u' => {
                    let mut hex = String::new();
                    for _ in 0..4 {
                        if let Some(h) = chars.next() {
                            if h.is_ascii_hexdigit() {
                                hex.push(h);
                            } else {
                                break;
                            }
                        }
                    }
                    if hex.len() == 4 {
                        if let Ok(code) = u32::from_str_radix(&hex, 16) {
                            if let Some(scalar) = char::from_u32(code) {
                                result.push(scalar);
                            } else {
                                result.push_str("\\u");
                                result.push_str(&hex);
                            }
                        } else {
                            result.push_str("\\u");
                            result.push_str(&hex);
                        }
                    } else {
                        result.push_str("\\u");
                        result.push_str(&hex);
                    }
                }
                other => result.push(other),
            }
        } else {
            result.push(ch);
        }
    }
    if let Ok(parsed) = JSONParser::parse(&result) {
        return parsed.format(2, false, false);
    }
    result
}

// MARK: - Parser

#[derive(Clone, Debug, PartialEq)]
pub struct JSONParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl JSONParseError {
    pub fn new(message: String, line: usize, column: usize) -> Self {
        Self { message, line, column }
    }
}

impl std::fmt::Display for JSONParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at line {}, column {}", self.message, self.line, self.column)
    }
}

impl std::error::Error for JSONParseError {}

pub struct JSONParser {
    bytes: Vec<u8>,
    index: usize,
    count: usize,
}

impl JSONParser {
    pub fn parse(text: &str) -> Result<JSONValue, JSONParseError> {
        let mut p = JSONParser { bytes: text.as_bytes().to_vec(), index: 0, count: text.len() };
        p.count = p.bytes.len();
        p.parse_root()
    }

    fn parse_root(&mut self) -> Result<JSONValue, JSONParseError> {
        self.skip_whitespace();
        if self.index >= self.count {
            return Err(self.make_error("Empty JSON input", self.index));
        }
        let value = self.parse_value()?;
        self.skip_whitespace();
        if self.index < self.count {
            let ch = self.bytes[self.index] as char;
            return Err(self.make_error(&format!("Unexpected character after valid JSON: '{}'", ch), self.index));
        }
        Ok(value)
    }

    fn parse_value(&mut self) -> Result<JSONValue, JSONParseError> {
        self.skip_whitespace();
        if self.index >= self.count {
            return Err(self.make_error("Unexpected end of JSON input", self.index));
        }
        let b = self.bytes[self.index];
        match b {
            b'{' => self.parse_object(),
            b'[' => self.parse_array(),
            b'"' | b'\'' => self.parse_string_value(),
            b't' | b'f' => self.parse_bool(),
            b'n' => self.parse_null(),
            b'-' | b'0'..=b'9' => self.parse_number(),
            _ => Err(self.make_error(&format!("Unexpected character '{}' when expecting value", b as char), self.index)),
        }
    }

    fn parse_object(&mut self) -> Result<JSONValue, JSONParseError> {
        let open_pos = self.index;
        self.index += 1;
        self.skip_whitespace();
        let mut pairs: Vec<JSONProperty> = Vec::with_capacity(8);
        if self.index < self.count && self.bytes[self.index] == b'}' {
            self.index += 1;
            return Ok(JSONValue::Object(pairs));
        }
        loop {
            if self.index >= self.count {
                return Err(self.make_error("Unclosed object, expected key or '}'", open_pos));
            }
            self.skip_whitespace();
            if self.index >= self.count {
                return Err(self.make_error("Unclosed object, expected key or '}'", open_pos));
            }
            if self.bytes[self.index] == b'}' {
                self.index += 1;
                return Ok(JSONValue::Object(pairs));
            }
            let b = self.bytes[self.index];
            let key: String = if b == b'"' || b == b'\'' {
                self.parse_raw_string()?
            } else if b.is_ascii_alphabetic() || b == b'_' || b == b'$' {
                self.parse_identifier()
            } else {
                return Err(self.make_error("Expected string key in object", self.index));
            };
            self.skip_whitespace();
            if self.index >= self.count || self.bytes[self.index] != b':' {
                return Err(self.make_error(&format!("Expected ':' after key '{}'", key), self.index.min(self.count)));
            }
            self.index += 1;
            let val = self.parse_value()?;
            pairs.push(JSONProperty::new(key, val));
            self.skip_whitespace();
            if self.index < self.count && self.bytes[self.index] == b',' {
                self.index += 1;
                self.skip_whitespace();
                if self.index < self.count && self.bytes[self.index] == b'}' {
                    self.index += 1;
                    return Ok(JSONValue::Object(pairs));
                }
            } else if self.index < self.count && self.bytes[self.index] == b'}' {
                self.index += 1;
                return Ok(JSONValue::Object(pairs));
            } else {
                return Err(self.make_error("Expected ',' or '}' in object", self.index.min(self.count)));
            }
        }
    }

    fn parse_array(&mut self) -> Result<JSONValue, JSONParseError> {
        let open_pos = self.index;
        self.index += 1;
        self.skip_whitespace();
        let mut items: Vec<JSONValue> = Vec::with_capacity(16);
        if self.index < self.count && self.bytes[self.index] == b']' {
            self.index += 1;
            return Ok(JSONValue::Array(items));
        }
        loop {
            if self.index >= self.count {
                return Err(self.make_error("Unterminated array: missing ']'", open_pos));
            }
            self.skip_whitespace();
            if self.index >= self.count {
                return Err(self.make_error("Unterminated array: missing ']'", open_pos));
            }
            if self.bytes[self.index] == b']' {
                self.index += 1;
                return Ok(JSONValue::Array(items));
            }
            let val = self.parse_value()?;
            items.push(val);
            self.skip_whitespace();
            if self.index >= self.count {
                return Err(self.make_error("Unterminated array: missing ']'", open_pos));
            }
            if self.bytes[self.index] == b',' {
                self.index += 1;
                self.skip_whitespace();
                if self.index < self.count && self.bytes[self.index] == b']' {
                    self.index += 1;
                    return Ok(JSONValue::Array(items));
                }
            } else if self.bytes[self.index] == b']' {
                self.index += 1;
                return Ok(JSONValue::Array(items));
            } else {
                return Err(self.make_error("Expected ',' or ']' in array", self.index));
            }
        }
    }

    fn parse_string_value(&mut self) -> Result<JSONValue, JSONParseError> {
        Ok(JSONValue::Str(self.parse_raw_string()?))
    }

    fn parse_raw_string(&mut self) -> Result<String, JSONParseError> {
        let quote = self.bytes[self.index];
        self.index += 1;
        let start = self.index;
        let mut has_escape = false;
        while self.index < self.count {
            let b = self.bytes[self.index];
            if b == quote {
                let s = String::from_utf8_lossy(&self.bytes[start..self.index]).to_string();
                self.index += 1;
                if !has_escape {
                    return Ok(s);
                } else {
                    return Self::decode_escapes(&s, self.index);
                }
            } else if b == b'\\' {
                has_escape = true;
                self.index += 1;
                if self.index < self.count {
                    self.index += 1;
                }
            } else if b == b'\n' || b == b'\r' {
                return Err(self.make_error("Unescaped newline in string literal", self.index));
            } else {
                self.index += 1;
            }
        }
        Err(self.make_error("Unterminated string", start.saturating_sub(1)))
    }

    fn decode_escapes(s: &str, pos: usize) -> Result<String, JSONParseError> {
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                let next = chars.next().ok_or_else(|| JSONParseError::new("Unterminated escape sequence".to_string(), 1, pos))?;
                match next {
                    '"' | '\\' | '/' => result.push(next),
                    'b' => result.push('\u{08}'),
                    'f' => result.push('\u{0C}'),
                    'n' => result.push('\n'),
                    'r' => result.push('\r'),
                    't' => result.push('\t'),
                    'u' => {
                        let mut hex = String::new();
                        for _ in 0..4 {
                            match chars.next() {
                                Some(h) if h.is_ascii_hexdigit() => hex.push(h),
                                _ => return Err(JSONParseError::new("Invalid unicode escape in string".to_string(), 1, pos)),
                            }
                        }
                        match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                            Some(c) => result.push(c),
                            None => result.push('?'),
                        }
                    }
                    other => result.push(other),
                }
            } else {
                result.push(ch);
            }
        }
        Ok(result)
    }

    fn parse_identifier(&mut self) -> String {
        let start = self.index;
        while self.index < self.count {
            let b = self.bytes[self.index];
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b == b'-' {
                self.index += 1;
            } else {
                break;
            }
        }
        String::from_utf8_lossy(&self.bytes[start..self.index]).to_string()
    }

    fn parse_bool(&mut self) -> Result<JSONValue, JSONParseError> {
        if self.index + 4 <= self.count
            && self.bytes[self.index] == b't'
            && self.bytes[self.index + 1] == b'r'
            && self.bytes[self.index + 2] == b'u'
            && self.bytes[self.index + 3] == b'e'
        {
            self.index += 4;
            return Ok(JSONValue::Bool(true));
        }
        if self.index + 5 <= self.count
            && self.bytes[self.index] == b'f'
            && self.bytes[self.index + 1] == b'a'
            && self.bytes[self.index + 2] == b'l'
            && self.bytes[self.index + 3] == b's'
            && self.bytes[self.index + 4] == b'e'
        {
            self.index += 5;
            return Ok(JSONValue::Bool(false));
        }
        Err(self.make_error("Invalid boolean literal", self.index))
    }

    fn parse_null(&mut self) -> Result<JSONValue, JSONParseError> {
        if self.index + 4 <= self.count
            && self.bytes[self.index] == b'n'
            && self.bytes[self.index + 1] == b'u'
            && self.bytes[self.index + 2] == b'l'
            && self.bytes[self.index + 3] == b'l'
        {
            self.index += 4;
            return Ok(JSONValue::Null);
        }
        Err(self.make_error("Invalid null literal", self.index))
    }

    fn parse_number(&mut self) -> Result<JSONValue, JSONParseError> {
        let start = self.index;
        if self.index < self.count && self.bytes[self.index] == b'-' {
            self.index += 1;
        }
        let mut has_digits = false;
        while self.index < self.count && self.bytes[self.index].is_ascii_digit() {
            has_digits = true;
            self.index += 1;
        }
        if self.index < self.count && self.bytes[self.index] == b'.' {
            self.index += 1;
            while self.index < self.count && self.bytes[self.index].is_ascii_digit() {
                has_digits = true;
                self.index += 1;
            }
        }
        if self.index < self.count && (self.bytes[self.index] == b'e' || self.bytes[self.index] == b'E') {
            self.index += 1;
            if self.index < self.count && (self.bytes[self.index] == b'+' || self.bytes[self.index] == b'-') {
                self.index += 1;
            }
            while self.index < self.count && self.bytes[self.index].is_ascii_digit() {
                self.index += 1;
            }
        }
        if !has_digits {
            return Err(self.make_error("Invalid number", start));
        }
        let num_str = String::from_utf8_lossy(&self.bytes[start..self.index]).to_string();
        match num_str.parse::<f64>() {
            Ok(d) => Ok(JSONValue::Number(d, num_str)),
            Err(_) => Err(self.make_error(&format!("Failed to parse number '{}'", num_str), start)),
        }
    }

    fn skip_whitespace(&mut self) {
        while self.index < self.count {
            let b = self.bytes[self.index];
            if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
                self.index += 1;
            } else if b == b'/' {
                if self.index + 1 < self.count && self.bytes[self.index + 1] == b'/' {
                    self.index += 2;
                    while self.index < self.count && self.bytes[self.index] != b'\n' {
                        self.index += 1;
                    }
                } else if self.index + 1 < self.count && self.bytes[self.index + 1] == b'*' {
                    self.index += 2;
                    while self.index + 1 < self.count {
                        if self.bytes[self.index] == b'*' && self.bytes[self.index + 1] == b'/' {
                            self.index += 2;
                            break;
                        }
                        self.index += 1;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    fn make_error(&self, message: &str, pos: usize) -> JSONParseError {
        let mut line: usize = 1;
        let mut col: usize = 1;
        let limit = pos.min(self.count);
        for i in 0..limit {
            if self.bytes[i] == b'\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        JSONParseError::new(message.to_string(), line, col)
    }
}
