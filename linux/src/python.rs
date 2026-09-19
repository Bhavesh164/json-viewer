//! Python literal parser — Rust port of
//! `Sources/JSONViewerCore/PythonLiteralParser.swift`.
//! Converts Python dict/literal syntax into [`crate::json::JSONValue`].

use crate::json::{JSONProperty, JSONValue};

#[derive(Clone, Debug, PartialEq)]
pub enum ParseError {
    UnexpectedEOF,
    UnexpectedCharacter(char, usize),
    InvalidKey(usize),
    ExpectedColon(usize),
    UnmatchedBracket(char, usize),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::UnexpectedEOF => write!(f, "Unexpected end of input while parsing Python literal."),
            ParseError::UnexpectedCharacter(ch, pos) => write!(f, "Unexpected character '{}' at character offset {}.", ch, pos),
            ParseError::InvalidKey(pos) => write!(f, "Expected dictionary key string or identifier at offset {}.", pos),
            ParseError::ExpectedColon(pos) => write!(f, "Expected ':' after dictionary key at offset {}.", pos),
            ParseError::UnmatchedBracket(ch, pos) => write!(f, "Unmatched bracket '{}' at offset {}.", ch, pos),
        }
    }
}

impl std::error::Error for ParseError {}

struct Parser {
    chars: Vec<char>,
    index: usize,
    offset: usize,
}

impl Parser {
    fn new(text: &str) -> Self {
        Self { chars: text.chars().collect(), index: 0, offset: 0 }
    }

    fn has_more(&self) -> bool {
        self.index < self.chars.len()
    }

    fn current(&self) -> char {
        if self.has_more() { self.chars[self.index] } else { '\0' }
    }

    fn advance(&mut self) {
        if self.has_more() {
            self.index += 1;
            self.offset += 1;
        }
    }

    fn peek(&self, n: usize) -> Option<char> {
        self.chars.get(self.index + n).copied()
    }

    fn skip_ws_and_comments(&mut self) {
        while self.has_more() {
            let ch = self.current();
            if ch.is_whitespace() {
                self.advance();
            } else if ch == '#' {
                while self.has_more() && self.current() != '\n' && self.current() != '\r' {
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    fn parse_value(&mut self) -> Result<JSONValue, ParseError> {
        self.skip_ws_and_comments();
        if !self.has_more() {
            return Err(ParseError::UnexpectedEOF);
        }
        let ch = self.current();
        match ch {
            '{' => self.parse_dict(),
            '[' => self.parse_list(']'),
            '(' => self.parse_list(')'),
            '"' | '\'' => self.parse_string(),
            'r' | 'u' | 'b' | 'R' | 'U' | 'B' => {
                if let Some(next) = self.peek(1) {
                    if next == '\'' || next == '"' {
                        self.advance();
                        return self.parse_string();
                    }
                }
                self.parse_ident_or_keyword()
            }
            '-' | '+' | '0'..='9' => self.parse_number(),
            _ => self.parse_ident_or_keyword(),
        }
    }

    fn parse_dict(&mut self) -> Result<JSONValue, ParseError> {
        self.advance();
        self.skip_ws_and_comments();
        let mut props: Vec<JSONProperty> = Vec::new();
        while self.has_more() && self.current() != '}' {
            self.skip_ws_and_comments();
            if !self.has_more() {
                break;
            }
            if self.current() == '}' {
                break;
            }
            let key_start = self.offset;
            let key: String = if self.current() == '\'' || self.current() == '"' {
                match self.parse_string()? {
                    JSONValue::Str(s) => s,
                    _ => return Err(ParseError::InvalidKey(key_start)),
                }
            } else if is_ident_start(self.current()) {
                self.parse_ident_name()
            } else {
                return Err(ParseError::InvalidKey(self.offset));
            };
            self.skip_ws_and_comments();
            if !self.has_more() || self.current() != ':' {
                return Err(ParseError::ExpectedColon(self.offset));
            }
            self.advance();
            self.skip_ws_and_comments();
            let val = self.parse_value()?;
            props.push(JSONProperty::new(key, val));
            self.skip_ws_and_comments();
            if !self.has_more() {
                break;
            }
            if self.current() == ',' {
                self.advance();
                self.skip_ws_and_comments();
            } else if self.current() == '}' {
                break;
            } else {
                return Err(ParseError::UnexpectedCharacter(self.current(), self.offset));
            }
        }
        if !self.has_more() || self.current() != '}' {
            return Err(ParseError::UnmatchedBracket('}', self.offset));
        }
        self.advance();
        Ok(JSONValue::Object(props))
    }

    fn parse_list(&mut self, closing: char) -> Result<JSONValue, ParseError> {
        self.advance();
        self.skip_ws_and_comments();
        let mut elements: Vec<JSONValue> = Vec::new();
        while self.has_more() && self.current() != closing {
            self.skip_ws_and_comments();
            if !self.has_more() {
                break;
            }
            if self.current() == closing {
                break;
            }
            let val = self.parse_value()?;
            elements.push(val);
            self.skip_ws_and_comments();
            if !self.has_more() {
                break;
            }
            if self.current() == ',' {
                self.advance();
                self.skip_ws_and_comments();
            } else if self.current() == closing {
                break;
            } else {
                return Err(ParseError::UnexpectedCharacter(self.current(), self.offset));
            }
        }
        if !self.has_more() || self.current() != closing {
            return Err(ParseError::UnmatchedBracket(closing, self.offset));
        }
        self.advance();
        Ok(JSONValue::Array(elements))
    }

    fn parse_string(&mut self) -> Result<JSONValue, ParseError> {
        let quote = self.current();
        self.advance();
        let mut is_triple = false;
        if self.has_more() && self.current() == quote && self.peek(1) == Some(quote) {
            self.advance();
            self.advance();
            is_triple = true;
        }
        let mut result = String::new();
        while self.has_more() {
            let ch = self.current();
            if ch == '\\' {
                self.advance();
                if !self.has_more() {
                    return Err(ParseError::UnexpectedEOF);
                }
                let esc = self.current();
                match esc {
                    'n' => result.push('\n'),
                    'r' => result.push('\r'),
                    't' => result.push('\t'),
                    '\\' => result.push('\\'),
                    '\'' => result.push('\''),
                    '"' => result.push('"'),
                    '/' => result.push('/'),
                    'b' => result.push('\u{0008}'),
                    'f' => result.push('\u{000C}'),
                    'u' => {
                        let mut hex = String::new();
                        for _ in 0..4 {
                            self.advance();
                            if self.has_more() {
                                hex.push(self.current());
                            }
                        }
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
                    }
                    other => result.push(other),
                }
                self.advance();
            } else if is_triple {
                if ch == quote && self.peek(1) == Some(quote) && self.peek(2) == Some(quote) {
                    self.advance();
                    self.advance();
                    self.advance();
                    return Ok(JSONValue::Str(result));
                } else {
                    result.push(ch);
                    self.advance();
                }
            } else if ch == quote {
                self.advance();
                return Ok(JSONValue::Str(result));
            } else {
                result.push(ch);
                self.advance();
            }
        }
        Err(ParseError::UnexpectedEOF)
    }

    fn parse_number(&mut self) -> Result<JSONValue, ParseError> {
        let mut num = String::new();
        while self.has_more() {
            let ch = self.current();
            if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+' || ch == 'e' || ch == 'E' {
                num.push(ch);
                self.advance();
            } else if ch == '_' {
                self.advance();
            } else {
                break;
            }
        }
        match num.parse::<f64>() {
            Ok(d) => Ok(JSONValue::Number(d, num)),
            Err(_) => Err(ParseError::UnexpectedCharacter(self.current(), self.offset)),
        }
    }

    fn parse_ident_or_keyword(&mut self) -> Result<JSONValue, ParseError> {
        let name = self.parse_ident_name();
        match name.as_str() {
            "True" | "true" => Ok(JSONValue::Bool(true)),
            "False" | "false" => Ok(JSONValue::Bool(false)),
            "None" | "null" | "none" => Ok(JSONValue::Null),
            "nan" | "NaN" => Ok(JSONValue::Null),
            _ => Ok(JSONValue::Str(name)),
        }
    }

    fn parse_ident_name(&mut self) -> String {
        let mut name = String::new();
        while self.has_more() {
            let ch = self.current();
            if ch.is_alphabetic() || ch.is_ascii_digit() || ch == '_' || ch == '$' {
                name.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        name
    }
}

fn is_ident_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_' || ch == '$'
}

pub fn parse_python_literal(text: &str) -> Result<JSONValue, ParseError> {
    let mut cleaned = text.trim().to_string();
    if let Some(eq_idx) = cleaned.find('=') {
        let prefix = cleaned[..eq_idx].trim();
        let valid = !prefix.is_empty()
            && prefix.chars().all(|c| c.is_alphabetic() || c.is_ascii_digit() || c == '_' || c == ':' || c.is_whitespace());
        if valid {
            let suffix = cleaned[eq_idx + 1..].trim().to_string();
            if suffix.starts_with('{') || suffix.starts_with('[') || suffix.starts_with('(') {
                cleaned = suffix;
            }
        }
    }
    let mut parser = Parser::new(&cleaned);
    parser.skip_ws_and_comments();
    if !parser.has_more() {
        return Err(ParseError::UnexpectedEOF);
    }
    let val = parser.parse_value()?;
    parser.skip_ws_and_comments();
    Ok(val)
}
