import Foundation

/// High-performance parser that converts Python dictionary and literal syntax into `JSONValue`.
/// Handles single quotes (`'`), `True`, `False`, `None`, tuples `(...)`, trailing commas,
/// unquoted dictionary keys, `#` comments, and numeric literals.
public struct PythonLiteralParser {
    
    public enum ParseError: Error, LocalizedError, Equatable {
        case unexpectedEOF
        case unexpectedCharacter(Character, Int)
        case invalidKey(Int)
        case expectedColon(Int)
        case unmatchedBracket(Character, Int)
        
        public var errorDescription: String? {
            switch self {
            case .unexpectedEOF:
                return "Unexpected end of input while parsing Python literal."
            case .unexpectedCharacter(let ch, let pos):
                return "Unexpected character '\(ch)' at character offset \(pos)."
            case .invalidKey(let pos):
                return "Expected dictionary key string or identifier at offset \(pos)."
            case .expectedColon(let pos):
                return "Expected ':' after dictionary key at offset \(pos)."
            case .unmatchedBracket(let ch, let pos):
                return "Unmatched bracket '\(ch)' at offset \(pos)."
            }
        }
    }
    
    private let text: String
    private var index: String.Index
    private var offset: Int = 0
    
    private init(_ text: String) {
        self.text = text
        self.index = text.startIndex
    }
    
    public static func parse(_ text: String) throws -> JSONValue {
        var parser = PythonLiteralParser(text)
        parser.skipWhitespaceAndComments()
        guard parser.hasMore else {
            throw ParseError.unexpectedEOF
        }
        let val = try parser.parseValue()
        parser.skipWhitespaceAndComments()
        return val
    }
    
    // MARK: - Parser State Helpers
    
    private var hasMore: Bool {
        index < text.endIndex
    }
    
    private var current: Character {
        guard hasMore else { return "\0" }
        return text[index]
    }
    
    private mutating func advance() {
        if hasMore {
            index = text.index(after: index)
            offset += 1
        }
    }
    
    private mutating func peek(offsetBy n: Int) -> Character? {
        guard let idx = text.index(index, offsetBy: n, limitedBy: text.index(before: text.endIndex)) else {
            return nil
        }
        return text[idx]
    }
    
    private mutating func skipWhitespaceAndComments() {
        while hasMore {
            let ch = current
            if ch.isWhitespace {
                advance()
            } else if ch == "#" {
                // Skip Python comment until newline
                while hasMore && current != "\n" && current != "\r" {
                    advance()
                }
            } else {
                break
            }
        }
    }
    
    // MARK: - Value Parsing
    
    private mutating func parseValue() throws -> JSONValue {
        skipWhitespaceAndComments()
        guard hasMore else { throw ParseError.unexpectedEOF }
        
        let ch = current
        switch ch {
        case "{":
            return try parseDictionary()
        case "[":
            return try parseList(closingChar: "]")
        case "(":
            return try parseList(closingChar: ")")
        case "\"", "'":
            return try parseString()
        case "r", "u", "b", "R", "U", "B":
            if let next = peek(offsetBy: 1), next == "'" || next == "\"" {
                advance() // skip prefix
                return try parseString()
            }
            return try parseIdentifierOrKeyword()
        case "-", "+", "0"..."9":
            return try parseNumber()
        default:
            return try parseIdentifierOrKeyword()
        }
    }
    
    // MARK: - Dictionary Parsing
    
    private mutating func parseDictionary() throws -> JSONValue {
        // Consume '{'
        advance()
        skipWhitespaceAndComments()
        
        var properties: [JSONProperty] = []
        
        while hasMore && current != "}" {
            skipWhitespaceAndComments()
            if current == "}" { break }
            
            // Key: can be quoted string or unquoted identifier
            let keyStartOffset = offset
            let key: String
            if current == "'" || current == "\"" {
                if case .string(let s) = try parseString() {
                    key = s
                } else {
                    throw ParseError.invalidKey(keyStartOffset)
                }
            } else if isIdentifierStart(current) {
                key = parseIdentifierName()
            } else {
                throw ParseError.invalidKey(offset)
            }
            
            skipWhitespaceAndComments()
            guard current == ":" else {
                throw ParseError.expectedColon(offset)
            }
            advance() // Consume ':'
            
            skipWhitespaceAndComments()
            let val = try parseValue()
            properties.append(JSONProperty(key: key, value: val))
            
            skipWhitespaceAndComments()
            if current == "," {
                advance() // Consume ','
                skipWhitespaceAndComments()
            } else if current == "}" {
                break
            } else {
                throw ParseError.unexpectedCharacter(current, offset)
            }
        }
        
        guard current == "}" else {
            throw ParseError.unmatchedBracket("}", offset)
        }
        advance() // Consume '}'
        
        return .object(properties)
    }
    
    // MARK: - List & Tuple Parsing
    
    private mutating func parseList(closingChar: Character) throws -> JSONValue {
        advance() // Consume opening '[' or '('
        skipWhitespaceAndComments()
        
        var elements: [JSONValue] = []
        
        while hasMore && current != closingChar {
            skipWhitespaceAndComments()
            if current == closingChar { break }
            
            let val = try parseValue()
            elements.append(val)
            
            skipWhitespaceAndComments()
            if current == "," {
                advance() // Consume ','
                skipWhitespaceAndComments()
            } else if current == closingChar {
                break
            } else {
                throw ParseError.unexpectedCharacter(current, offset)
            }
        }
        
        guard current == closingChar else {
            throw ParseError.unmatchedBracket(closingChar, offset)
        }
        advance() // Consume closing char
        
        return .array(elements)
    }
    
    // MARK: - String Parsing
    
    private mutating func parseString() throws -> JSONValue {
        let quote = current
        advance() // Consume opening quote
        
        // Check for triple quote
        var isTriple = false
        if hasMore && current == quote && peek(offsetBy: 1) == quote {
            advance()
            advance()
            isTriple = true
        }
        
        var result = ""
        while hasMore {
            let ch = current
            if ch == "\\" {
                advance()
                guard hasMore else { throw ParseError.unexpectedEOF }
                let esc = current
                switch esc {
                case "n": result.append("\n")
                case "r": result.append("\r")
                case "t": result.append("\t")
                case "\\": result.append("\\")
                case "'": result.append("'")
                case "\"": result.append("\"")
                case "/": result.append("/")
                case "b": result.append("\u{0008}")
                case "f": result.append("\u{000C}")
                case "u":
                    // Unicode 4 hex digits
                    var hex = ""
                    for _ in 0..<4 {
                        advance()
                        if hasMore { hex.append(current) }
                    }
                    if let code = UInt32(hex, radix: 16), let scalar = UnicodeScalar(code) {
                        result.append(Character(scalar))
                    } else {
                        result.append("\\u" + hex)
                    }
                default:
                    result.append(esc)
                }
                advance()
            } else if isTriple {
                if ch == quote && peek(offsetBy: 1) == quote && peek(offsetBy: 2) == quote {
                    advance()
                    advance()
                    advance()
                    return .string(result)
                } else {
                    result.append(ch)
                    advance()
                }
            } else {
                if ch == quote {
                    advance() // Consume closing quote
                    return .string(result)
                } else {
                    result.append(ch)
                    advance()
                }
            }
        }
        
        throw ParseError.unexpectedEOF
    }
    
    // MARK: - Number Parsing
    
    private mutating func parseNumber() throws -> JSONValue {
        var numStr = ""
        while hasMore {
            let ch = current
            if ch.isNumber || ch == "." || ch == "-" || ch == "+" || ch == "e" || ch == "E" {
                numStr.append(ch)
                advance()
            } else if ch == "_" {
                // Python allows underscores in numbers like 1_000_000
                advance()
            } else {
                break
            }
        }
        
        guard let d = Double(numStr) else {
            throw ParseError.unexpectedCharacter(current, offset)
        }
        return .number(d, raw: numStr)
    }
    
    // MARK: - Identifier / Keyword Parsing
    
    private mutating func parseIdentifierOrKeyword() throws -> JSONValue {
        let name = parseIdentifierName()
        switch name {
        case "True", "true":
            return .bool(true)
        case "False", "false":
            return .bool(false)
        case "None", "null", "none":
            return .null
        case "nan", "NaN":
            return .null
        default:
            // Return as string value if recognized as literal token
            return .string(name)
        }
    }
    
    private mutating func parseIdentifierName() -> String {
        var name = ""
        while hasMore {
            let ch = current
            if ch.isLetter || ch.isNumber || ch == "_" || ch == "$" {
                name.append(ch)
                advance()
            } else {
                break
            }
        }
        return name
    }
    
    private func isIdentifierStart(_ ch: Character) -> Bool {
        ch.isLetter || ch == "_" || ch == "$"
    }
}
