import Foundation
import SwiftUI

/// Identifiable tree node representing a point in the JSON hierarchy.
public final class JSONNode: Identifiable, ObservableObject, @unchecked Sendable {
    public let id: String
    public let key: String
    public let value: JSONValue
    public let path: String
    public var children: [JSONNode]?
    public weak var parent: JSONNode?
    
    public init(
        id: String? = nil,
        key: String,
        value: JSONValue,
        path: String,
        children: [JSONNode]? = nil,
        parent: JSONNode? = nil
    ) {
        self.id = id ?? path
        self.key = key
        self.value = value
        self.path = path
        self.children = children
        self.parent = parent
    }
    
    public var isLeaf: Bool {
        children == nil || children!.isEmpty
    }
    
    public var isContainer: Bool {
        !isLeaf
    }
    
    /// Display text matching jsonviewer.stack.hu node styling:
    /// - leaf: "key : value" (e.g. `age : 28`, `name : "Alice"`)
    /// - container: "key" or "key [N items]"
    public var displayText: String {
        switch value {
        case .null:
            return "\(key) : null"
        case .bool(let b):
            return "\(key) : \(b ? "true" : "false")"
        case .number(_, let raw):
            return "\(key) : \(raw)"
        case .string(let s):
            return "\(key) : \"\(s)\""
        case .array(let items):
            if key == "JSON" {
                return "JSON [\(items.count)]"
            }
            return "\(key) [\(items.count)]"
        case .object(let pairs):
            if key == "JSON" {
                return "JSON {\(pairs.count)}"
            }
            return "\(key) {\(pairs.count)}"
        }
    }
    
    /// Pure value string for copying or table display
    public var valueString: String {
        switch value {
        case .null:
            return "null"
        case .bool(let b):
            return b ? "true" : "false"
        case .number(_, let raw):
            return raw
        case .string(let s):
            return s
        case .array:
            return "[\(children?.count ?? 0) items]"
        case .object:
            return "{\(children?.count ?? 0) properties}"
        }
    }
    
    public var formattedValueString: String {
        switch value {
        case .null:
            return "null"
        case .bool(let b):
            return b ? "true" : "false"
        case .number(_, let raw):
            return raw
        case .string(let s):
            return "\"\(s)\""
        case .array, .object:
            return value.format(indentSpaces: 2)
        }
    }
    
    public var typeColor: Color {
        switch value {
        case .string:
            return Color(red: 0.05, green: 0.45, blue: 0.95) // Blue
        case .number:
            return Color(red: 0.15, green: 0.65, blue: 0.35) // Green
        case .bool:
            return Color(red: 0.90, green: 0.55, blue: 0.05) // Yellow / Orange
        case .null:
            return Color(red: 0.85, green: 0.20, blue: 0.20) // Red
        case .array:
            return Color(red: 0.40, green: 0.35, blue: 0.85) // Indigo
        case .object:
            return Color(red: 0.55, green: 0.25, blue: 0.80) // Purple
        }
    }
    
    public var systemIconName: String {
        switch value {
        case .string:
            return "textformat"
        case .number:
            return "number"
        case .bool:
            return "switch.2"
        case .null:
            return "circle.slash"
        case .array:
            return "square.stack.3d.down.right.fill"
        case .object:
            return "curlybraces"
        }
    }
    
    public var typeBadgeText: String {
        switch value {
        case .string: return "str"
        case .number: return "num"
        case .bool: return "bool"
        case .null: return "null"
        case .array(let items): return "[\(items.count)]"
        case .object(let pairs): return "{\(pairs.count)}"
        }
    }
    
    /// Produces direct property list for the Property Grid.
    /// Matches jsonviewer.stack.hu: if the selected node is a leaf, it displays its parent's properties!
    public func propertiesForGrid() -> [PropertyGridRow] {
        let containerNode = isContainer ? self : (parent ?? self)
        guard let children = containerNode.children else {
            return [PropertyGridRow(id: 0, name: key, value: valueString, type: value.typeName, path: path, nodeId: id)]
        }
        
        return children.enumerated().map { idx, child in
            PropertyGridRow(
                id: idx,
                name: child.key,
                value: child.valueString,
                type: child.value.typeName,
                path: child.path,
                nodeId: child.id
            )
        }
    }
    
    /// Recursively search all nodes matching the query
    public func searchMatches(query: String) -> [JSONNode] {
        let lowerQuery = query.lowercased()
        var matches: [JSONNode] = []
        
        if displayText.lowercased().contains(lowerQuery) || path.lowercased().contains(lowerQuery) {
            matches.append(self)
        }
        
        if let children = children {
            for child in children {
                matches.append(contentsOf: child.searchMatches(query: query))
            }
        }
        
        return matches
    }
    
    /// Path of ancestor node IDs to expand to reveal this node
    public var ancestorIds: [String] {
        var result: [String] = []
        var curr = parent
        while let p = curr {
            result.append(p.id)
            curr = p.parent
        }
        return result
    }
}

public struct FlatTreeRow: Identifiable, @unchecked Sendable {
    public var id: String { node.id }
    public let node: JSONNode
    public let depth: Int
    public let isExpanded: Bool
    public let isContainer: Bool
    
    public init(node: JSONNode, depth: Int, isExpanded: Bool) {
        self.node = node
        self.depth = depth
        self.isExpanded = isExpanded
        self.isContainer = node.isContainer
    }
}

public struct PropertyGridRow: Identifiable, Equatable, Sendable {
    public let id: Int
    public let name: String
    public let value: String
    public let type: String
    public let path: String
    public let nodeId: String
    
    public init(id: Int = 0, name: String, value: String, type: String, path: String, nodeId: String = "") {
        self.id = id
        self.name = name
        self.value = value
        self.type = type
        self.path = path
        self.nodeId = nodeId
    }
}

// MARK: - Tree Builder
extension JSONNode {
    public static func buildTree(from json: JSONValue, rootKey: String = "JSON") -> JSONNode {
        let root = buildNode(key: rootKey, value: json, path: "$", parent: nil)
        return root
    }
    
    private static func buildNode(key: String, value: JSONValue, path: String, parent: JSONNode?) -> JSONNode {
        switch value {
        case .object(let pairs):
            let node = JSONNode(key: key, value: value, path: path, children: nil, parent: parent)
            let builtChildren = pairs.map { pair in
                let childPath = "\(path).\(pair.key)"
                return buildNode(key: pair.key, value: pair.value, path: childPath, parent: node)
            }
            node.children = builtChildren
            return node
            
        case .array(let items):
            let node = JSONNode(key: key, value: value, path: path, children: nil, parent: parent)
            let builtChildren = items.enumerated().map { idx, item in
                let childPath = "\(path)[\(idx)]"
                return buildNode(key: "\(idx)", value: item, path: childPath, parent: node)
            }
            node.children = builtChildren
            return node
            
        default:
            return JSONNode(key: key, value: value, path: path, children: nil, parent: parent)
        }
    }
}
