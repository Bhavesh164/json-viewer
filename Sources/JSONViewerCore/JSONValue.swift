import Foundation

public struct JSONProperty: Equatable, Sendable {
    public let key: String
    public let value: JSONValue
    
    public init(key: String, value: JSONValue) {
        self.key = key
        self.value = value
    }
}

/// Represents any valid JSON value while preserving object key ordering.
public enum JSONValue: Equatable, Sendable {
    case null
    case bool(Bool)
    case number(Double, raw: String)
    case string(String)
    case array([JSONValue])
    case object([JSONProperty])
    
    public var typeName: String {
        switch self {
        case .null: return "null"
        case .bool: return "boolean"
        case .number: return "number"
        case .string: return "string"
        case .array: return "array"
        case .object: return "object"
        }
    }
    
    public var isContainer: Bool {
        switch self {
        case .object, .array: return true
        default: return false
        }
    }
    
    public var childCount: Int {
        switch self {
        case .object(let pairs): return pairs.count
        case .array(let items): return items.count
        default: return 0
        }
    }
    
    public var summary: String {
        switch self {
        case .null:
            return "null"
        case .bool(let b):
            return b ? "true" : "false"
        case .number(_, let raw):
            return raw
        case .string(let s):
            return "\"\(s)\""
        case .array(let items):
            return "Array [\(items.count)]"
        case .object(let pairs):
            return "Object {\(pairs.count)}"
        }
    }
}

// MARK: - Formatting & Minification
extension JSONValue {
    /// Format the JSON tree into pretty-printed string with specified indentation (default 2 spaces matching jsonviewer.stack.hu)
    public func format(indentSpaces: Int = 2) -> String {
        var output = ""
        format(into: &output, currentIndent: 0, indentSpaces: indentSpaces)
        return output
    }
    
    private func format(into output: inout String, currentIndent: Int, indentSpaces: Int) {
        let indent = String(repeating: " ", count: currentIndent)
        let childIndent = String(repeating: " ", count: currentIndent + indentSpaces)
        
        switch self {
        case .null:
            output.append("null")
        case .bool(let b):
            output.append(b ? "true" : "false")
        case .number(_, let raw):
            output.append(raw)
        case .string(let str):
            output.append(Self.escapeString(str))
        case .array(let items):
            if items.isEmpty {
                output.append("[]")
                return
            }
            output.append("[\n")
            for (idx, item) in items.enumerated() {
                output.append(childIndent)
                item.format(into: &output, currentIndent: currentIndent + indentSpaces, indentSpaces: indentSpaces)
                if idx < items.count - 1 {
                    output.append(",")
                }
                output.append("\n")
            }
            output.append(indent + "]")
        case .object(let pairs):
            if pairs.isEmpty {
                output.append("{}")
                return
            }
            output.append("{\n")
            for (idx, pair) in pairs.enumerated() {
                output.append(childIndent)
                output.append(Self.escapeString(pair.key))
                output.append(": ")
                pair.value.format(into: &output, currentIndent: currentIndent + indentSpaces, indentSpaces: indentSpaces)
                if idx < pairs.count - 1 {
                    output.append(",")
                }
                output.append("\n")
            }
            output.append(indent + "}")
        }
    }
    
    /// Minify JSON into compact representation without whitespace outside strings
    public func minify() -> String {
        var output = ""
        minify(into: &output)
        return output
    }
    
    private func minify(into output: inout String) {
        switch self {
        case .null:
            output.append("null")
        case .bool(let b):
            output.append(b ? "true" : "false")
        case .number(_, let raw):
            output.append(raw)
        case .string(let str):
            output.append(Self.escapeString(str))
        case .array(let items):
            output.append("[")
            for (idx, item) in items.enumerated() {
                item.minify(into: &output)
                if idx < items.count - 1 {
                    output.append(",")
                }
            }
            output.append("]")
        case .object(let pairs):
            output.append("{")
            for (idx, pair) in pairs.enumerated() {
                output.append(Self.escapeString(pair.key))
                output.append(":")
                pair.value.minify(into: &output)
                if idx < pairs.count - 1 {
                    output.append(",")
                }
            }
            output.append("}")
        }
    }
    
    private static func escapeString(_ str: String) -> String {
        var escaped = "\""
        for ch in str {
            switch ch {
            case "\"": escaped.append("\\\"")
            case "\\": escaped.append("\\\\")
            case "\t": escaped.append("\\t")
            case "\n": escaped.append("\\n")
            case "\r": escaped.append("\\r")
            case "\u{08}": escaped.append("\\b")
            case "\u{0C}": escaped.append("\\f")
            default:
                if ch.unicodeScalars.first!.value < 0x20 {
                    escaped.append(String(format: "\\u%04x", ch.unicodeScalars.first!.value))
                } else {
                    escaped.append(ch)
                }
            }
        }
        escaped.append("\"")
        return escaped
    }
}

// MARK: - JSON Parser with Order Preservation & Error Reporting
public struct JSONParseError: Error, LocalizedError, Equatable, Sendable {
    public let message: String
    public let line: Int
    public let column: Int
    
    public var errorDescription: String? {
        "\(message) at line \(line), column \(column)"
    }
}

public struct JSONParser {
    private let bytes: [UInt8]
    private var index: Int = 0
    private let count: Int
    
    public init(text: String) {
        self.bytes = Array(text.utf8)
        self.count = bytes.count
    }
    
    public static func parse(_ text: String) throws -> JSONValue {
        var parser = JSONParser(text: text)
        return try parser.parseRoot()
    }
    
    public mutating func parseRoot() throws -> JSONValue {
        skipWhitespace()
        guard index < count else {
            throw makeError("Empty JSON input", at: index)
        }
        let value = try parseValue()
        skipWhitespace()
        if index < count {
            let charStr = String(UnicodeScalar(bytes[index]))
            throw makeError("Unexpected character after valid JSON: '\(charStr)'", at: index)
        }
        return value
    }
    
    private mutating func parseValue() throws -> JSONValue {
        skipWhitespace()
        guard index < count else {
            throw makeError("Unexpected end of JSON input", at: index)
        }
        
        let b = bytes[index]
        switch b {
        case 0x7B: // '{'
            return try parseObject()
        case 0x5B: // '['
            return try parseArray()
        case 0x22, 0x27: // '"', '\''
            return try parseStringValue()
        case 0x74, 0x66: // 't', 'f'
            return try parseBool()
        case 0x6E: // 'n'
            return try parseNull()
        case 0x2D, 0x30...0x39: // '-', '0'...'9'
            return try parseNumber()
        default:
            let charStr = String(UnicodeScalar(b))
            throw makeError("Unexpected character '\(charStr)' when expecting value", at: index)
        }
    }
    
    private mutating func parseObject() throws -> JSONValue {
        let openPos = index
        index += 1 // skip '{'
        skipWhitespace()
        
        var pairs: [JSONProperty] = []
        pairs.reserveCapacity(8)
        
        if index < count && bytes[index] == 0x7D { // '}'
            index += 1
            return .object(pairs)
        }
        
        while index < count {
            skipWhitespace()
            guard index < count else {
                throw makeError("Unclosed object, expected key or '}'", at: openPos)
            }
            
            if bytes[index] == 0x7D { // '}'
                index += 1
                return .object(pairs)
            }
            
            // Parse Key (either quoted string or unquoted JS identifier for resilience)
            let key: String
            let b = bytes[index]
            if b == 0x22 || b == 0x27 { // '"', '\''
                key = try parseRawString()
            } else if (b >= 0x41 && b <= 0x5A) || (b >= 0x61 && b <= 0x7A) || b == 0x5F || b == 0x24 { // A-Z, a-z, '_', '$'
                key = try parseIdentifier()
            } else {
                throw makeError("Expected string key in object", at: index)
            }
            
            skipWhitespace()
            guard index < count, bytes[index] == 0x3A else { // ':'
                throw makeError("Expected ':' after key '\(key)'", at: index)
            }
            index += 1 // skip ':'
            
            let val = try parseValue()
            pairs.append(JSONProperty(key: key, value: val))
            
            skipWhitespace()
            if index < count && bytes[index] == 0x2C { // ','
                index += 1
                skipWhitespace()
                // Handle trailing comma supportively
                if index < count && bytes[index] == 0x7D { // '}'
                    index += 1
                    return .object(pairs)
                }
            } else if index < count && bytes[index] == 0x7D { // '}'
                index += 1
                return .object(pairs)
            } else {
                throw makeError("Expected ',' or '}' in object", at: index)
            }
        }
        
        throw makeError("Unterminated object: missing '}'", at: openPos)
    }
    
    private mutating func parseArray() throws -> JSONValue {
        let openPos = index
        index += 1 // skip '['
        skipWhitespace()
        
        var items: [JSONValue] = []
        items.reserveCapacity(16)
        
        if index < count && bytes[index] == 0x5D { // ']'
            index += 1
            return .array(items)
        }
        
        while index < count {
            skipWhitespace()
            if bytes[index] == 0x5D { // ']'
                index += 1
                return .array(items)
            }
            
            let val = try parseValue()
            items.append(val)
            
            skipWhitespace()
            if index < count && bytes[index] == 0x2C { // ','
                index += 1
                skipWhitespace()
                // Handle trailing comma supportively
                if index < count && bytes[index] == 0x5D { // ']'
                    index += 1
                    return .array(items)
                }
            } else if index < count && bytes[index] == 0x5D { // ']'
                index += 1
                return .array(items)
            } else {
                throw makeError("Expected ',' or ']' in array", at: index)
            }
        }
        
        throw makeError("Unterminated array: missing ']'", at: openPos)
    }
    
    private mutating func parseStringValue() throws -> JSONValue {
        let str = try parseRawString()
        return .string(str)
    }
    
    private mutating func parseRawString() throws -> String {
        let quote = bytes[index]
        index += 1
        let start = index
        var hasEscape = false
        
        while index < count {
            let b = bytes[index]
            if b == quote {
                let s = String(decoding: bytes[start..<index], as: UTF8.self)
                index += 1
                if !hasEscape {
                    return s
                } else {
                    return try decodeEscapes(s)
                }
            } else if b == 0x5C { // '\\'
                hasEscape = true
                index += 1
                if index < count {
                    index += 1
                }
            } else if b == 0x0A || b == 0x0D {
                throw makeError("Unescaped newline in string literal", at: index)
            } else {
                index += 1
            }
        }
        
        throw makeError("Unterminated string", at: start - 1)
    }
    
    private func decodeEscapes(_ s: String) throws -> String {
        var result = ""
        var iter = s.makeIterator()
        while let ch = iter.next() {
            if ch == "\\" {
                guard let next = iter.next() else {
                    throw makeError("Unterminated escape sequence", at: index)
                }
                switch next {
                case "\"", "\\", "/":
                    result.append(next)
                case "b":
                    result.append("\u{08}")
                case "f":
                    result.append("\u{0C}")
                case "n":
                    result.append("\n")
                case "r":
                    result.append("\r")
                case "t":
                    result.append("\t")
                case "u":
                    var hex = ""
                    for _ in 0..<4 {
                        guard let h = iter.next(), h.isHexDigit else {
                            throw makeError("Invalid unicode escape in string", at: index)
                        }
                        hex.append(h)
                    }
                    if let code = UInt32(hex, radix: 16), let scalar = UnicodeScalar(code) {
                        result.append(Character(scalar))
                    } else {
                        result.append("?")
                    }
                default:
                    result.append(next)
                }
            } else {
                result.append(ch)
            }
        }
        return result
    }
    
    private mutating func parseIdentifier() throws -> String {
        let start = index
        while index < count {
            let b = bytes[index]
            if (b >= 0x41 && b <= 0x5A) || (b >= 0x61 && b <= 0x7A) || (b >= 0x30 && b <= 0x39) || b == 0x5F || b == 0x24 || b == 0x2D {
                index += 1
            } else {
                break
            }
        }
        return String(decoding: bytes[start..<index], as: UTF8.self)
    }
    
    private mutating func parseBool() throws -> JSONValue {
        if index + 4 <= count && bytes[index] == 0x74 && bytes[index+1] == 0x72 && bytes[index+2] == 0x75 && bytes[index+3] == 0x65 {
            index += 4
            return .bool(true)
        }
        if index + 5 <= count && bytes[index] == 0x66 && bytes[index+1] == 0x61 && bytes[index+2] == 0x6C && bytes[index+3] == 0x73 && bytes[index+4] == 0x65 {
            index += 5
            return .bool(false)
        }
        throw makeError("Invalid boolean literal", at: index)
    }
    
    private mutating func parseNull() throws -> JSONValue {
        if index + 4 <= count && bytes[index] == 0x6E && bytes[index+1] == 0x75 && bytes[index+2] == 0x6C && bytes[index+3] == 0x6C {
            index += 4
            return .null
        }
        throw makeError("Invalid null literal", at: index)
    }
    
    private mutating func parseNumber() throws -> JSONValue {
        let start = index
        if index < count && bytes[index] == 0x2D { // '-'
            index += 1
        }
        
        var hasDigits = false
        while index < count && (bytes[index] >= 0x30 && bytes[index] <= 0x39) {
            hasDigits = true
            index += 1
        }
        
        if index < count && bytes[index] == 0x2E { // '.'
            index += 1
            while index < count && (bytes[index] >= 0x30 && bytes[index] <= 0x39) {
                hasDigits = true
                index += 1
            }
        }
        
        if index < count && (bytes[index] == 0x65 || bytes[index] == 0x45) { // 'e', 'E'
            index += 1
            if index < count && (bytes[index] == 0x2B || bytes[index] == 0x2D) { // '+', '-'
                index += 1
            }
            while index < count && (bytes[index] >= 0x30 && bytes[index] <= 0x39) {
                index += 1
            }
        }
        
        guard hasDigits else {
            throw makeError("Invalid number", at: start)
        }
        
        let numStr = String(decoding: bytes[start..<index], as: UTF8.self)
        guard let d = Double(numStr) else {
            throw makeError("Failed to parse number '\(numStr)'", at: start)
        }
        return .number(d, raw: numStr)
    }
    
    private mutating func skipWhitespace() {
        while index < count {
            let b = bytes[index]
            if b == 0x20 || b == 0x09 || b == 0x0A || b == 0x0D { // ' ', '\t', '\n', '\r'
                index += 1
            } else if b == 0x2F { // '/'
                // Comments: // or /* */
                if index + 1 < count && bytes[index + 1] == 0x2F {
                    index += 2
                    while index < count && bytes[index] != 0x0A {
                        index += 1
                    }
                } else if index + 1 < count && bytes[index + 1] == 0x2A {
                    index += 2
                    while index + 1 < count {
                        if bytes[index] == 0x2A && bytes[index + 1] == 0x2F {
                            index += 2
                            break
                        }
                        index += 1
                    }
                } else {
                    break
                }
            } else {
                break
            }
        }
    }
    
    private func makeError(_ message: String, at pos: Int) -> JSONParseError {
        var line = 1
        var col = 1
        let limit = min(pos, count)
        for i in 0..<limit {
            if bytes[i] == 0x0A {
                line += 1
                col = 1
            } else {
                col += 1
            }
        }
        return JSONParseError(message: message, line: line, column: col)
    }
}
