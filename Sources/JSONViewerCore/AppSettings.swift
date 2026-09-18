import Foundation
import SwiftUI

public final class AppSettings: ObservableObject {
    public static let shared = AppSettings()
    
    private let defaults = UserDefaults.standard
    
    // Keys
    private enum Keys {
        static let indentSpaces = "settings_indent_spaces"
        static let sortKeys = "settings_sort_keys"
        static let escapeSlashes = "settings_escape_slashes"
        static let autoUnwrapStringified = "settings_auto_unwrap_stringified"
        static let defaultTab = "settings_default_tab"
        static let fontSize = "settings_font_size"
        static let wrapLines = "settings_wrap_lines"
    }
    
    @Published public var indentSpaces: Int {
        didSet { defaults.set(indentSpaces, forKey: Keys.indentSpaces) }
    }
    
    @Published public var sortKeysAlphabetically: Bool {
        didSet { defaults.set(sortKeysAlphabetically, forKey: Keys.sortKeys) }
    }
    
    @Published public var escapeSlashesInStringify: Bool {
        didSet { defaults.set(escapeSlashesInStringify, forKey: Keys.escapeSlashes) }
    }
    
    @Published public var autoUnwrapStringified: Bool {
        didSet { defaults.set(autoUnwrapStringified, forKey: Keys.autoUnwrapStringified) }
    }
    
    @Published public var defaultTab: String {
        didSet { defaults.set(defaultTab, forKey: Keys.defaultTab) }
    }
    
    @Published public var fontSize: CGFloat {
        didSet { defaults.set(Double(fontSize), forKey: Keys.fontSize) }
    }
    
    @Published public var wrapLines: Bool {
        didSet { defaults.set(wrapLines, forKey: Keys.wrapLines) }
    }
    
    public init() {
        let d = UserDefaults.standard
        
        // Default values
        let savedIndent = d.object(forKey: Keys.indentSpaces) != nil ? d.integer(forKey: Keys.indentSpaces) : 2
        self.indentSpaces = savedIndent
        
        self.sortKeysAlphabetically = d.bool(forKey: Keys.sortKeys)
        
        let savedEscape = d.object(forKey: Keys.escapeSlashes) != nil ? d.bool(forKey: Keys.escapeSlashes) : true
        self.escapeSlashesInStringify = savedEscape
        
        let savedAutoUnwrap = d.object(forKey: Keys.autoUnwrapStringified) != nil ? d.bool(forKey: Keys.autoUnwrapStringified) : true
        self.autoUnwrapStringified = savedAutoUnwrap
        
        self.defaultTab = d.string(forKey: Keys.defaultTab) ?? "Text"
        
        let savedFontSize = d.double(forKey: Keys.fontSize)
        self.fontSize = savedFontSize > 0 ? CGFloat(savedFontSize) : 12.0
        
        self.wrapLines = d.bool(forKey: Keys.wrapLines)
    }
    
    public func resetToDefaults() {
        indentSpaces = 2
        sortKeysAlphabetically = false
        escapeSlashesInStringify = true
        autoUnwrapStringified = true
        defaultTab = "Text"
        fontSize = 12.0
        wrapLines = false
    }
}
