import Foundation
import SwiftUI
import AppKit

public enum AppTab: String, CaseIterable, Identifiable {
    case viewer = "Viewer"
    case text = "Text"
    case split = "Split"
    
    public var id: String { rawValue }
}

@MainActor
public final class JSONDocumentModel: ObservableObject {
    @Published public var rawText: String = "" {
        didSet {
            isDirty = true
            updateTextMetrics()
        }
    }
    
    @Published public var activeTab: AppTab = .text
    @Published public var jsonValue: JSONValue?
    @Published public var rootNode: JSONNode?
    @Published public var selectedNode: JSONNode? {
        didSet {
            updateSelectedNodeProperties()
        }
    }
    @Published public var expandedNodeIds: Set<String> = []
    @Published public var expandedLeafNodeIds: Set<String> = []
    @Published public var visibleTreeRows: [FlatTreeRow] = []
    @Published public var selectedNodeProperties: [PropertyGridRow] = []
    @Published public var fontSize: CGFloat = 12.0
    
    // Zoom / Font Scaling
    public func zoomIn() {
        if fontSize < 24.0 {
            fontSize += 1.0
        }
    }
    
    public func zoomOut() {
        if fontSize > 9.0 {
            fontSize -= 1.0
        }
    }
    
    public func resetZoom() {
        fontSize = 12.0
    }
    
    // Parsing & Error State
    @Published public var parseError: JSONParseError?
    @Published public var isErrorAlertPresented: Bool = false
    @Published public var errorMessage: String = ""
    
    // Search State
    @Published public var isSearchVisible: Bool = true
    @Published public var searchQuery: String = ""
    @Published public var searchResults: [JSONNode] = []
    @Published public var searchResultIds: Set<String> = []
    @Published public var currentSearchIndex: Int = 0
    @Published public var searchStatus: String = ""
    @Published public var lastExecutedSearchQuery: String = ""
    
    // Sheets & UI Panels
    @Published public var isAboutSheetPresented: Bool = false
    @Published public var isPropertiesVisible: Bool = true
    
    // Status metrics
    public var isDirty: Bool = false
    @Published public private(set) var characterCount: Int = 0
    @Published public private(set) var lineCount: Int = 1
    
    private var metricsWorkItem: DispatchWorkItem?
    
    public func updateTextMetrics(immediate: Bool = false) {
        metricsWorkItem?.cancel()
        let text = rawText
        
        if text.isEmpty {
            self.characterCount = 0
            self.lineCount = 1
            return
        }
        
        // Fast path for small documents (< 15KB) or synchronous requests
        if immediate || text.count < 15_000 {
            let chars = (text as NSString).length
            var count = 1
            for byte in text.utf8 {
                if byte == 0x0A { count += 1 }
            }
            self.characterCount = chars
            self.lineCount = count
            return
        }
        
        // Debounce calculation off the main thread during continuous typing
        let work = DispatchWorkItem { [weak self] in
            guard let self = self else { return }
            let chars = (text as NSString).length
            var count = 1
            for byte in text.utf8 {
                if byte == 0x0A { count += 1 }
            }
            DispatchQueue.main.async {
                self.characterCount = chars
                self.lineCount = count
            }
        }
        metricsWorkItem = work
        DispatchQueue.global(qos: .userInitiated).asyncAfter(deadline: .now() + 0.12, execute: work)
    }
    
    public init() {
        // Provide sample JSON to show immediate value
        let sample = """
        {
          "title": "JSON Viewer macOS",
          "version": "1.0.0",
          "description": "Native Mac JSON Viewer & Formatter",
          "active": true,
          "rating": 4.95,
          "nullProperty": null,
          "author": {
            "name": "Antigravity & Stack.hu",
            "email": "local@mac.internal"
          },
          "features": [
            "Hierarchical Tree View",
            "Property Grid Inspection",
            "2-Space Indented Formatting",
            "Whitespace Minification",
            "Remote URL Loading",
            "Full Key & Value Search"
          ],
          "statistics": {
            "downloads": 12840,
            "stars": 892
          }
        }
        """
        self.rawText = sample
        self.parseAndBuildTree(silent: true)
    }
    
    // MARK: - Tab Switching & Validation
    public func selectTab(_ tab: AppTab) {
        activeTab = tab
        if (tab == .viewer || tab == .split) && (rootNode == nil || isDirty) {
            DispatchQueue.main.async {
                _ = self.parseAndBuildTree(silent: false)
            }
        }
    }
    
    @discardableResult
    public func parseAndBuildTree(silent: Bool = false) -> Bool {
        let trimmed = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            if !silent {
                showError("JSON error: Please enter JSON code in the Text tab first.")
            }
            return false
        }
        
        do {
            let parsed = try JSONParser.parse(trimmed)
            self.jsonValue = parsed
            let root = JSONNode.buildTree(from: parsed, rootKey: "JSON")
            self.rootNode = root
            self.selectedNode = root
            
            // Show initially like classic JSON Viewer: ONLY root node expanded, children collapsed
            self.expandedNodeIds = [root.id]
            self.updateVisibleRows()
            self.updateSelectedNodeProperties()
            self.parseError = nil
            self.isDirty = false
            return true
        } catch let err as JSONParseError {
            self.parseError = err
            if !silent {
                showError("JSON error: Invalid JSON variable\n\n\(err.message) at line \(err.line), col \(err.column)")
            }
            return false
        } catch {
            if !silent {
                showError("JSON error: \(error.localizedDescription)")
            }
            return false
        }
    }
    
    public func showError(_ message: String) {
        self.errorMessage = message
        self.isErrorAlertPresented = true
    }
    
    // MARK: - UI Panels
    public func toggleProperties() {
        isPropertiesVisible.toggle()
    }
    
    public func clearText() {
        rawText = ""
        jsonValue = nil
        rootNode = nil
        selectedNode = nil
        parseError = nil
        clearSearch()
    }
    
    // MARK: - Clipboard Operations
    public func copyText() {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(rawText, forType: .string)
    }
    
    public func pasteText() {
        let pasteboard = NSPasteboard.general
        if let string = pasteboard.string(forType: .string) {
            rawText = string
        }
    }
    
    // MARK: - Search
    public func clearSearch() {
        searchQuery = ""
        searchResults = []
        searchResultIds = []
        searchStatus = ""
        lastExecutedSearchQuery = ""
        currentSearchIndex = 0
    }
    
    public func searchStart() {
        let query = searchQuery.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else {
            clearSearch()
            return
        }
        
        lastExecutedSearchQuery = query
        
        guard let root = rootNode else {
            // Try building tree first
            if parseAndBuildTree(silent: true), let r = rootNode {
                performSearch(on: r, query: query)
            } else {
                searchStatus = "Phrase not found!"
            }
            return
        }
        
        performSearch(on: root, query: query)
    }
    
    private func performSearch(on root: JSONNode, query: String) {
        let matches = root.searchMatches(query: query)
        self.searchResults = matches
        self.searchResultIds = Set(matches.map { $0.id })
        
        if matches.isEmpty {
            self.searchStatus = "Phrase not found!"
        } else {
            self.currentSearchIndex = 0
            selectMatch(at: 0)
        }
    }
    
    public func searchNext() {
        let query = searchQuery.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return }
        
        if query != lastExecutedSearchQuery || searchResults.isEmpty {
            searchStart()
            return
        }
        
        guard !searchResults.isEmpty else { return }
        currentSearchIndex = (currentSearchIndex + 1) % searchResults.count
        selectMatch(at: currentSearchIndex)
    }
    
    public func searchPrevious() {
        let query = searchQuery.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return }
        
        if query != lastExecutedSearchQuery || searchResults.isEmpty {
            searchStart()
            if !searchResults.isEmpty {
                currentSearchIndex = searchResults.count - 1
                selectMatch(at: currentSearchIndex)
            }
            return
        }
        
        guard !searchResults.isEmpty else { return }
        currentSearchIndex = (currentSearchIndex - 1 + searchResults.count) % searchResults.count
        selectMatch(at: currentSearchIndex)
    }
    
    /// Called on Enter or Shift+Enter in search field
    public func searchSubmit(reverse: Bool = false) {
        if reverse {
            searchPrevious()
        } else {
            searchNext()
        }
    }
    
    private func selectMatch(at index: Int) {
        guard index >= 0 && index < searchResults.count else { return }
        let target = searchResults[index]
        self.selectedNode = target
        
        // Expand all ancestors to make node visible (only rebuild rows if newly expanded!)
        var didExpandAncestors = false
        for ancestorId in target.ancestorIds {
            if !expandedNodeIds.contains(ancestorId) {
                expandedNodeIds.insert(ancestorId)
                didExpandAncestors = true
            }
        }
        if didExpandAncestors {
            updateVisibleRows()
        }
        
        self.searchStatus = "\(index + 1) of \(searchResults.count) matches"
    }
    
    // MARK: - Tree Expansion Controls
    public var isAllExpanded: Bool {
        guard let root = rootNode else { return false }
        if let children = root.children {
            let containers = children.filter { $0.isContainer }
            if containers.isEmpty { return expandedNodeIds.contains(root.id) }
            return containers.allSatisfy { expandedNodeIds.contains($0.id) }
        }
        return expandedNodeIds.contains(root.id)
    }
    
    public func toggleExpandAll() {
        if isAllExpanded {
            collapseAll()
        } else {
            expandAll()
        }
    }
    
    public func expandAll() {
        guard let root = rootNode else { return }
        var allIds: Set<String> = []
        collectContainerIds(from: root, into: &allIds)
        expandedNodeIds = allIds
        updateVisibleRows()
    }
    
    public func collapseAll() {
        if let root = rootNode {
            expandedNodeIds = [root.id]
        } else {
            expandedNodeIds.removeAll()
        }
        updateVisibleRows()
    }
    
    public func expandSubtree(_ n: JSONNode) {
        collectContainerIds(from: n, into: &expandedNodeIds)
        updateVisibleRows()
    }
    
    public func collapseSubtree(_ n: JSONNode) {
        var subIds: Set<String> = []
        collectContainerIds(from: n, into: &subIds)
        expandedNodeIds.subtract(subIds)
        updateVisibleRows()
    }
    
    private func collectContainerIds(from node: JSONNode, into set: inout Set<String>) {
        if node.isContainer {
            set.insert(node.id)
            if let children = node.children {
                for child in children {
                    collectContainerIds(from: child, into: &set)
                }
            }
        }
    }
    
    public func toggleExpand(nodeId: String) {
        if expandedNodeIds.contains(nodeId) {
            expandedNodeIds.remove(nodeId)
        } else {
            expandedNodeIds.insert(nodeId)
        }
        updateVisibleRows()
    }
    
    public func toggleExpandLeaf(nodeId: String) {
        if expandedLeafNodeIds.contains(nodeId) {
            expandedLeafNodeIds.remove(nodeId)
        } else {
            expandedLeafNodeIds.insert(nodeId)
        }
    }
    
    public func updateVisibleRows() {
        guard let root = rootNode else {
            visibleTreeRows = []
            return
        }
        
        var rows: [FlatTreeRow] = []
        func traverse(node: JSONNode, depth: Int) {
            let isExp = expandedNodeIds.contains(node.id)
            rows.append(FlatTreeRow(node: node, depth: depth, isExpanded: isExp))
            if isExp, let children = node.children {
                for child in children {
                    traverse(node: child, depth: depth + 1)
                }
            }
        }
        traverse(node: root, depth: 0)
        self.visibleTreeRows = rows
    }
    
    public func updateSelectedNodeProperties() {
        if let selected = selectedNode {
            self.selectedNodeProperties = selected.propertiesForGrid()
        } else {
            self.selectedNodeProperties = []
        }
    }
    
    // MARK: - Node Navigation from Property Grid
    public func navigateToProperty(_ row: PropertyGridRow) {
        // 1. Direct match among current container's children
        let container = (selectedNode?.isContainer == true ? selectedNode : selectedNode?.parent) ?? rootNode
        if let target = container?.children?.first(where: { (!row.nodeId.isEmpty && $0.id == row.nodeId) || $0.key == row.name }) {
            selectAndReveal(node: target)
            return
        }
        
        // 2. Lookup by unique nodeId if present
        if !row.nodeId.isEmpty, let target = findNode(byId: row.nodeId) {
            selectAndReveal(node: target)
            return
        }
        
        // 3. Fallback lookup by json path
        if let target = findNode(byPath: row.path) {
            selectAndReveal(node: target)
        }
    }
    
    public func selectAndReveal(node: JSONNode) {
        var didExpandAncestors = false
        for ancestorId in node.ancestorIds {
            if !expandedNodeIds.contains(ancestorId) {
                expandedNodeIds.insert(ancestorId)
                didExpandAncestors = true
            }
        }
        if didExpandAncestors {
            updateVisibleRows()
        }
        self.selectedNode = node
    }
    
    public func navigateToParent() {
        guard let parent = selectedNode?.parent else { return }
        selectAndReveal(node: parent)
    }
    
    public func findNode(byId targetId: String) -> JSONNode? {
        guard let root = rootNode else { return nil }
        return findNode(in: root, byId: targetId)
    }
    
    private func findNode(in current: JSONNode, byId targetId: String) -> JSONNode? {
        if current.id == targetId { return current }
        if let children = current.children {
            for child in children {
                if let found = findNode(in: child, byId: targetId) {
                    return found
                }
            }
        }
        return nil
    }
    
    public func findNode(byPath targetPath: String) -> JSONNode? {
        guard let root = rootNode else { return nil }
        return findNode(in: root, byPath: targetPath)
    }
    
    private func findNode(in current: JSONNode, byPath targetPath: String) -> JSONNode? {
        if current.path == targetPath { return current }
        if let children = current.children {
            for child in children {
                if let found = findNode(in: child, byPath: targetPath) {
                    return found
                }
            }
        }
        return nil
    }

    
    // MARK: - File I/O
    public func openFile(url: URL) {
        do {
            let data = try Data(contentsOf: url)
            if let str = String(data: data, encoding: .utf8) {
                self.rawText = str
                self.parseAndBuildTree(silent: true)
            }
        } catch {
            showError("Failed to open file: \(error.localizedDescription)")
        }
    }
    
    public func saveToFile(url: URL) {
        do {
            try rawText.write(to: url, atomically: true, encoding: .utf8)
            isDirty = false
        } catch {
            showError("Failed to save file: \(error.localizedDescription)")
        }
    }
}
