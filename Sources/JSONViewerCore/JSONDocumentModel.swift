import Foundation
import SwiftUI
import AppKit
#if canImport(Darwin)
import Darwin
#endif

public enum AppTab: String, CaseIterable, Identifiable {
    case viewer = "Viewer"
    case text = "Text"
    case split = "Split"
    
    public var id: String { rawValue }
}

/// Implemented by the view that owns the editable text (the native code editor).
///
/// The editor is authoritative while the user is typing: `NSTextView.string`
/// materialises a fresh copy of the whole document on every read, so reading it
/// per keystroke costs one full-document allocation per character typed. The
/// model therefore keeps only a `pendingTextSync` flag while the user types and
/// pulls the text through this protocol once, in `syncRawTextIfNeeded()`, when
/// something actually needs it.
@MainActor
public protocol JSONDocumentTextSource: AnyObject {
    /// The editor's current text. Reading it is expensive on large documents.
    var currentText: String { get }
    /// True while this editor has keyboard focus, used to pick the right source when
    /// more than one editor view exists.
    var isActiveEditor: Bool { get }
}

@MainActor
public final class JSONDocumentModel: ObservableObject {
    @Published public var rawText: String = "" {
        didSet {
            isDirty = true
            isDocumentEmpty = rawText.isEmpty
            if isApplyingEditorSync, let metrics = editorSuppliedMetrics {
                // The counts came from the editor for exactly this text, so adopting
                // them costs nothing and a rescan would be pure waste.
                editorSuppliedMetrics = nil
                lineCount = metrics.lines
                characterCount = metrics.characters
            } else {
                // Any other assignment is not the text the editor counted, so its
                // counts must not be reused.
                editorSuppliedMetrics = nil
                updateTextMetrics()
            }
            scheduleRebuildIfNeeded()
        }
    }
    
    @Published public private(set) var treeVersion: Int = 0
    /// Incremented after a node has been expanded and selected for tree navigation.
    @Published public private(set) var treeNavigationRequest: Int = 0
    /// Incremented when clearing search should return the tree viewport to its root.
    @Published public private(set) var treeScrollToTopRequest: Int = 0
    
    /// Persisted cursor selection and scroll offset for the text editor across tab switches.
    public var lastEditorSelectedRange: NSRange?
    public var lastEditorScrollOrigin: CGPoint?
    
    @Published public var activeTab: AppTab = .text {
        didSet {
            // Switching tabs must not parse. Like the Linux port, the tree is kept
            // current by the idle rebuild, so a switch is nothing but a view change:
            // no freeze, and no swap of rows that are already on screen.
            syncRawTextIfNeeded()
            guard activeTab == .viewer || activeTab == .split else { return }
            
            if isDocumentEmpty {
                // Nothing to show, and nothing to parse.
                if rootNode != nil || isDirty { discardTree() }
                showError("JSON error: Please enter JSON code in the Text tab first.")
                return
            }
            
            // A rebuild is still queued. The user is looking at the tree now, so do not
            // make them wait out the rest of the debounce.
            if isDirty && !isParsing {
                // Not silent: switching tabs has always reported a parse failure.
                runScheduledRebuild(silent: false)
            }
        }
    }
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
    public let settings: AppSettings
    @Published public var fontSize: CGFloat = 12.0 {
        didSet {
            settings.fontSize = fontSize
        }
    }
    
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
    @Published public var focusSearchFieldTrigger: Int = 0
    @Published public var searchQuery: String = "" {
        didSet {
            guard searchQuery != oldValue else { return }
            cancelSearchTask()
            searchResults = []
            searchResultIds = []
            searchStatus = ""
            lastExecutedSearchQuery = ""
            currentSearchIndex = 0
            if searchQuery.isEmpty {
                treeScrollToTopRequest += 1
            }
        }
    }
    @Published public var searchResults: [JSONNode] = []
    @Published public var searchResultIds: Set<String> = []
    @Published public var currentSearchIndex: Int = 0
    @Published public var searchStatus: String = ""
    @Published public var lastExecutedSearchQuery: String = ""
    private var searchTask: Task<Void, Never>?
    private var searchGeneration = 0
    
    public func focusSearch() {
        isSearchVisible = true
        focusSearchFieldTrigger += 1
    }
    
    // Sheets & UI Panels
    @Published public var isAboutSheetPresented: Bool = false
    @Published public var isShortcutsSheetPresented: Bool = false
    @Published public var isSettingsSheetPresented: Bool = false
    @Published public var isPropertiesVisible: Bool = true
    @Published public var copiedToastMessage: String? = nil
    
    // Status metrics
    @Published public var isDirty: Bool = false
    @Published public private(set) var characterCount: Int = 0
    @Published public private(set) var lineCount: Int = 1
    /// True while the document is empty, including edits that only the editor
    /// knows about. Views must not read `rawText` for this: it lags behind the
    /// editor by design while the user is typing.
    @Published public private(set) var isDocumentEmpty: Bool = true
    
    private var metricsWorkItem: DispatchWorkItem?
    private var metricsGeneration = 0
    
    /// Line and character counts the editor already knew for the text it holds.
    /// Supplying them keeps a keystroke from putting an O(document) rescan back on
    /// the typing path.
    private struct EditorMetrics {
        let lines: Int
        let characters: Int
    }
    private var editorSuppliedMetrics: EditorMetrics?
    
    public func updateTextMetrics(immediate: Bool = false) {
        metricsWorkItem?.cancel()
        metricsWorkItem = nil
        metricsGeneration += 1
        let generation = metricsGeneration
        let text = rawText
        
        if text.isEmpty {
            self.characterCount = 0
            self.lineCount = 1
            return
        }
        
        // The character count is the UTF-16 length, which is a stored property rather
        // than a walk, so it is always cheap and always correct. It also drives the
        // live-parse debounce, so it must not wait for the debounced line count or a
        // freshly opened large document would size its debounce from the old one.
        let chars = (text as NSString).length
        self.characterCount = chars
        
        // Fast path for small documents (< 15K characters) or synchronous requests.
        // NSString.length is used as the size probe because `text.count` walks
        // graphemes, which is a per-character pass over the whole document.
        if immediate || chars < 15_000 {
            self.lineCount = Self.lineCount(of: text)
            return
        }
        
        // Only the line count is expensive, so only it goes off the main thread.
        let work = DispatchWorkItem { [weak self] in
            let count = Self.lineCount(of: text)
            DispatchQueue.main.async {
                guard let self = self, self.metricsGeneration == generation else { return }
                self.lineCount = count
            }
        }
        metricsWorkItem = work
        DispatchQueue.global(qos: .userInitiated).asyncAfter(deadline: .now() + 0.12, execute: work)
    }
    
    /// Number of lines in `text`, matching the status bar: an empty document is
    /// one line, otherwise one plus the number of line feeds.
    ///
    /// This runs per keystroke for documents that are not small enough to take the
    /// synchronous path, so it counts with `memchr` over the UTF-8 buffer instead
    /// of iterating `String.UTF8View` byte by byte. `String.split` is *not* a
    /// memchr-backed alternative here — measured slower than the plain byte loop.
    nonisolated public static func lineCount(of text: String) -> Int {
        if text.isEmpty { return 1 }
        var text = text
        let newlines = text.withUTF8 { buffer -> Int in
            guard let base = buffer.baseAddress, !buffer.isEmpty else { return 0 }
            let newline = Int32(0x0A)
            let limit = UnsafeRawPointer(base + buffer.count)
            var cursor = UnsafeRawPointer(base)
            var total = 0
            while cursor < limit {
                guard let found = memchr(cursor, newline, limit - cursor) else { break }
                total += 1
                cursor = UnsafeRawPointer(found) + 1
            }
            return total
        }
        return 1 + newlines
    }
    
    // MARK: - Editor Text Synchronisation
    
    private var pendingTextSync = false
    /// True only while the assignment to `rawText` comes from pulling the editor's
    /// text in, which is the one assignment the editor's counts describe.
    private var isApplyingEditorSync = false
    
    /// Editors are held weakly — the view owns them — and a tab switch can briefly
    /// have two alive at once, so more than one may be registered.
    private final class WeakTextSource {
        weak var value: (any JSONDocumentTextSource)?
        init(_ value: any JSONDocumentTextSource) { self.value = value }
    }
    private var textSources: [WeakTextSource] = []
    private weak var lastEditedSource: (any JSONDocumentTextSource)?
    
    public func registerTextSource(_ source: any JSONDocumentTextSource) {
        // Drop the same source and any editor that is already gone, so the list does
        // not grow with every tab switch.
        textSources.removeAll { $0.value == nil || $0.value === source }
        textSources.append(WeakTextSource(source))
    }
    
    public func unregisterTextSource(_ source: any JSONDocumentTextSource) {
        // Flush before dropping the reference, otherwise edits the user just made
        // would be lost when the editor goes away.
        syncRawTextIfNeeded()
        textSources.removeAll { $0.value === source }
        if lastEditedSource === source { lastEditedSource = nil }
    }
    
    /// The editor to pull the text from: the one that produced the last edit wins,
    /// then whichever has keyboard focus, then the most recently registered.
    private var activeTextSource: (any JSONDocumentTextSource)? {
        if let edited = lastEditedSource { return edited }
        for box in textSources.reversed() {
            if let source = box.value, source.isActiveEditor { return source }
        }
        return textSources.reversed().compactMap { $0.value }.first
    }
    
    /// Remember which editor produced the last edit. Focus often moves on before the
    /// text is pulled in — clicking the tree, a toolbar button — and the edit is what
    /// matters, not whatever has focus at that point.
    public func noteTextSourceEdited(_ source: any JSONDocumentTextSource) {
        lastEditedSource = source
    }
    
    /// True while the editor holds edits that are not in `rawText` yet.
    public var hasPendingTextSync: Bool { pendingTextSync }
    
    /// Copy the editor's text into `rawText`, but only if it has changed since the
    /// last sync. Every consumer of the document text calls this first.
    @discardableResult
    public func syncRawTextIfNeeded() -> Bool {
        guard pendingTextSync else { return false }
        guard let source = activeTextSource else { return false }
        pendingTextSync = false
        lastEditedSource = source
        isApplyingEditorSync = true
        rawText = source.currentText
        isApplyingEditorSync = false
        return true
    }
    
    /// Record an edit made in the editor without copying its text.
    ///
    /// The line and character counts come from the editor, which knows them from
    /// its own bookkeeping, so the typing path never rescans the document. The
    /// text itself is only pulled in by `syncRawTextIfNeeded()`.
    public func markEditedFromEditor(lineCount: Int, characterCount: Int) {
        pendingTextSync = true
        editorSuppliedMetrics = EditorMetrics(lines: max(1, lineCount), characters: characterCount)
        isDirty = true
        self.lineCount = max(1, lineCount)
        self.characterCount = characterCount
        self.isDocumentEmpty = characterCount == 0
        // The editor's counts describe the text now in the buffer, so any in-flight
        // measurement of the previous text must not be allowed to overwrite them.
        metricsWorkItem?.cancel()
        metricsWorkItem = nil
        metricsGeneration += 1
        scheduleRebuildIfNeeded()
    }
    
    
    
    public init(settings: AppSettings = .shared) {
        self.settings = settings
        self.fontSize = settings.fontSize
        if let initialTab = AppTab(rawValue: settings.defaultTab) {
            self.activeTab = initialTab
        }
        
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
    /// Selecting a tab relies on `activeTab.didSet` to flush the editor's pending
    /// text and rebuild the tree, so the switch happens in one place whether the tab
    /// is picked from the toolbar or set directly.
    public func selectTab(_ tab: AppTab) {
        activeTab = tab
    }
    
    private var scheduledRebuild: DispatchWorkItem?
    private var rebuildTask: Task<Void, Never>?
    /// Number of rebuilds currently running in the background, so `isParsing` stays
    /// true until the last one lands even if some were cancelled.
    private var inFlightRebuilds = 0
    /// True while a background rebuild is outstanding. The UI uses it for feedback, and
    /// a tab switch uses it to avoid starting a rebuild that would be discarded.
    @Published public private(set) var isParsing: Bool = false
    /// Incremented every time a rebuild result is applied. Diagnostics, and a way for
    /// tests to wait for a rebuild rather than guess at the debounce.
    public private(set) var rebuildCount: Int = 0
    /// Bumped every time the document text is assigned from something other than the
    /// editor — a transform, Paste, Open, Clear, or a rewrite to standard JSON.
    ///
    /// The editor watches this instead of comparing strings: it only has to touch the
    /// buffer when the model has actually produced new text, which keeps a
    /// whole-document comparison off the SwiftUI update path.
    public private(set) var modelTextRevision: Int = 0
    /// Bumped whenever the document text changes or a parse starts, so a tree
    /// built off the main thread from stale text is discarded instead of applied.
    private var parseSerial = 0
    
    /// Rebuild the tree on idle, in every tab.
    ///
    /// This mirrors the Linux port, where `flush_debounced_reparse` is called from the
    /// Text tab as well as Split. Because the tree is rebuilt while the user is still
    /// looking at the editor, switching to the Viewer has nothing left to do: no parse,
    /// no freeze, and no swap of the rows that are already on screen — which is what
    /// made the tree appear to jump.
    private func scheduleRebuildIfNeeded() {
        cancelScheduledRebuild()
        // The text just changed, so any rebuild in flight is already out of date.
        parseSerial += 1
        let delay = rebuildDebounceDelay
        let work = DispatchWorkItem { [weak self] in
            self?.runScheduledRebuild()
        }
        scheduledRebuild = work
        DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: work)
    }
    
    private func cancelScheduledRebuild() {
        scheduledRebuild?.cancel()
        scheduledRebuild = nil
    }
    
    /// Rebuild the tree after the debounce has elapsed.
    private func runScheduledRebuild(silent: Bool = true) {
        scheduledRebuild = nil
        // Pulling the editor's text in reschedules a rebuild of exactly this text; that
        // is this rebuild.
        syncRawTextIfNeeded()
        cancelScheduledRebuild()
        startRebuild(silent: silent)
    }
    
    /// Snapshot the navigation state a rebuild has to carry over.
    private func restoreHints() -> RestoreHints {
        var hints = RestoreHints()
        hints.ids.formUnion(expandedNodeIds)
        hints.ids.formUnion(expandedLeafNodeIds)
        if let id = selectedNode?.id { hints.ids.insert(id) }
        if let path = selectedNode?.path { hints.paths.insert(path) }
        return hints
    }
    
    /// Rebuild the tree, off the main thread once the document is big enough that a
    /// synchronous rebuild would be felt.
    ///
    /// Returns true when the tree is already up to date on return; false means the
    /// work moved to a background task and `isParsing` is now true.
    @discardableResult
    private func startRebuild(silent: Bool) -> Bool {
        syncRawTextIfNeeded()
        cancelScheduledRebuild()
        parseSerial += 1
        let serial = parseSerial
        let text = rawText
        let options = currentParseOptions()
        let hints = restoreHints()
        
        guard Self.exceedsInlineParseLimit(text) else {
            let outcome = Self.parseDocument(text, options: options, restore: hints)
            return applyParse(outcome, silent: silent).success
        }
        
        rebuildTask?.cancel()
        beginRebuild()
        rebuildTask = Task.detached(priority: .userInitiated) { [weak self] in
            let outcome = JSONDocumentModel.parseDocument(text, options: options, restore: hints)
            if Task.isCancelled {
                // Balance the counter the cancelled task would otherwise never release,
                // so `isParsing` cannot get stuck on.
                await self?.endRebuild()
                return
            }
            await self?.finishBackgroundRebuild(outcome, serial: serial, silent: silent)
        }
        return true
    }
    
    /// Above this many characters a rebuild moves to a background task.
    ///
    /// Parse cost is close to linear in document size — measured at ~48 ms per MB on
    /// this machine — so a synchronous rebuild is only worth it while it stays under
    /// roughly one frame. 256K characters is that line: ~10 ms.
    nonisolated public static func exceedsInlineParseLimit(_ text: String) -> Bool {
        return (text as NSString).length > 262_144
    }
    
    private func beginRebuild() {
        inFlightRebuilds += 1
        isParsing = true
    }
    
    private func endRebuild() {
        inFlightRebuilds = max(0, inFlightRebuilds - 1)
        isParsing = inFlightRebuilds > 0
    }
    
    private func finishBackgroundRebuild(_ outcome: ParseOutcome, serial: Int, silent: Bool) {
        endRebuild()
        // A newer edit already asked for a rebuild; this result is of a text that no
        // longer exists.
        guard serial == parseSerial else { return }
        rebuildTask = nil
        // Rewriting the text (Python input, unescaped JSON) schedules another rebuild
        // of a document that parses to exactly this tree. Drop it.
        if applyParse(outcome, silent: silent).rewroteText {
            cancelScheduledRebuild()
        }
    }
    
    /// How long to wait after the last keystroke before rebuilding the tree.
    ///
    /// Parsing plus tree building is a few hundred milliseconds on a multi-megabyte
    /// document, so a fixed 350 ms delay means someone typing steadily still
    /// triggers a rebuild between keystrokes. The delay grows with the document, so
    /// a big file is only rebuilt after a real pause: 350 ms up to 1 MB, then
    /// +250 ms per MB, capped at 1.5 s.
    public var rebuildDebounceDelay: TimeInterval {
        let megabytes = Double(characterCount) / (1024.0 * 1024.0)
        let extra = min(1.15, megabytes * 0.25)
        return 0.35 + extra
    }
    
    /// Settings snapshot, so the parse can run without touching main-actor state.
    private struct ParseOptions: Sendable {
        var indentSpaces: Int
        var sortKeys: Bool
        var escapeSlashes: Bool
        var autoUnwrapStringified: Bool
    }
    
    private func currentParseOptions() -> ParseOptions {
        ParseOptions(
            indentSpaces: settings.indentSpaces,
            sortKeys: settings.sortKeysAlphabetically,
            escapeSlashes: settings.escapeSlashesInStringify,
            autoUnwrapStringified: settings.autoUnwrapStringified
        )
    }
    
    /// Everything one parse produced, computed away from the main thread.
    ///
    /// `JSONValue` and `JSONNode` are immutable once built, so handing them across
    /// threads is safe; that is why this is unchecked.
    private struct ParseOutcome: @unchecked Sendable {
        var value: JSONValue?
        var root: JSONNode?
        /// The nodes to carry over from the previous tree, resolved by the parse.
        var restored: RestoredNodes?
        /// Set when the source was Python or a stringified document, so the editor
        /// can be rewritten to standard JSON.
        var rewrittenText: String?
        var error: JSONParseError?
        var isEmptyDocument: Bool = false
    }
    
    /// Parse `text` and build its tree. Pure: no main-actor state is read or written,
    /// so it can run on a background thread.
    nonisolated private static func parseDocument(_ text: String, options: ParseOptions, restore: RestoreHints) -> ParseOutcome {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            return ParseOutcome(isEmptyDocument: true)
        }
        
        var rewrittenText: String?
        var parsed: JSONValue
        do {
            do {
                parsed = try JSONParser.parse(trimmed)
            } catch let initialErr {
                // 1. If direct parse failed, check if input is a Python dictionary or literal
                if let pythonParsed = try? PythonLiteralParser.parse(trimmed) {
                    parsed = pythonParsed
                    // Only a structured document is rewritten to standard JSON. A bare
                    // Python scalar is left alone on purpose: the rebuild also runs while
                    // the user is still typing, and quietly turning the word they are
                    // typing into a quoted string would edit the document under them.
                    if pythonParsed.isContainer {
                        rewrittenText = pythonParsed.format(
                            indentSpaces: options.indentSpaces,
                            sortKeys: options.sortKeys,
                            escapeSlashes: options.escapeSlashes
                        )
                    }
                } else if options.autoUnwrapStringified {
                    // 2. Attempt unescape in case of raw stringified JSON
                    let unescaped = JSONValue.unescapeStringifiedJSON(trimmed)
                    if unescaped != trimmed, let fallback = try? JSONParser.parse(unescaped) {
                        parsed = fallback
                    } else if unescaped != trimmed, let fallbackPython = try? PythonLiteralParser.parse(unescaped) {
                        parsed = fallbackPython
                        if fallbackPython.isContainer {
                            rewrittenText = fallbackPython.format(
                                indentSpaces: options.indentSpaces,
                                sortKeys: options.sortKeys,
                                escapeSlashes: options.escapeSlashes
                            )
                        }
                    } else {
                        throw initialErr
                    }
                } else {
                    throw initialErr
                }
            }
            
            // Auto-unwrap stringified JSON if enabled
            if options.autoUnwrapStringified, case .string(let innerStr) = parsed {
                let innerTrimmed = innerStr.trimmingCharacters(in: .whitespacesAndNewlines)
                if (innerTrimmed.hasPrefix("{") && innerTrimmed.hasSuffix("}")) || (innerTrimmed.hasPrefix("[") && innerTrimmed.hasSuffix("]")) {
                    if let innerParsed = try? JSONParser.parse(innerTrimmed) {
                        parsed = innerParsed
                    }
                }
            }
        } catch let err as JSONParseError {
            return ParseOutcome(error: err)
        } catch {
            return ParseOutcome(error: JSONParseError(message: error.localizedDescription, line: 1, column: 1))
        }
        
        let root = JSONNode.buildTree(from: parsed, rootKey: "JSON")
        return ParseOutcome(value: parsed, root: root, restored: restoreNodes(in: root, hints: restore), rewrittenText: rewrittenText)
    }
    
    /// The nodes a rebuild has to carry over from the tree it is replacing.
    ///
    /// This is a handful of ids and paths, not the whole tree. It is snapshotted on
    /// the main actor before the work starts, because the nodes it refers to belong
    /// to the *previous* tree and are about to be thrown away.
    private struct RestoreHints: Sendable {
        var ids: Set<String> = []
        var paths: Set<String> = []
    }
    
    /// Resolve just the nodes `hints` asks for, in one walk of the new tree.
    ///
    /// The obvious alternative — index every node by id and by path, plus a set of
    /// every container and every leaf — cost **0.59 s of the 1.0 s** parse of a
    /// 900,000-node document, all of it hashing nearly a million long path strings to
    /// look up one selection and filter two small sets. Because a rebuild normally has
    /// to restore only the root, the walk stops as soon as everything asked for has
    /// been found, which for the common case is after the first node.
    ///
    /// The worst case — restoring a state that names many nodes, such as after
    /// Expand All — walks the whole tree, which is still less work than indexing it.
    nonisolated private static func restoreNodes(in root: JSONNode, hints: RestoreHints) -> RestoredNodes {
        var byID: [String: JSONNode] = [:]
        var byPath: [String: JSONNode] = [:]
        var wantedIDs = hints.ids
        var wantedPaths = hints.paths
        if wantedIDs.isEmpty && wantedPaths.isEmpty { return RestoredNodes(byID: byID, byPath: byPath) }
        
        // Iterative, so a deeply nested document cannot overflow the stack here.
        var pending: [JSONNode] = [root]
        while let node = pending.popLast() {
            // `remove` reports whether it was there, which keeps a node that answers
            // to both an id and a path from being counted twice.
            if wantedIDs.remove(node.id) != nil { byID[node.id] = node }
            if wantedPaths.remove(node.path) != nil { byPath[node.path] = node }
            if wantedIDs.isEmpty && wantedPaths.isEmpty { break }
            if let children = node.children {
                pending.append(contentsOf: children)
            }
        }
        return RestoredNodes(byID: byID, byPath: byPath)
    }
    
    private struct RestoredNodes: @unchecked Sendable {
        var byID: [String: JSONNode]
        var byPath: [String: JSONNode]
    }
    
    /// Parse synchronously on the main actor. Used where the caller needs the tree to
    /// be correct on return, such as a transform that just rewrote the text.
    @discardableResult
    public func parseAndBuildTree(silent: Bool = false) -> Bool {
        syncRawTextIfNeeded()
        parseSerial += 1
        let outcome = Self.parseDocument(rawText, options: currentParseOptions(), restore: restoreHints())
        let result = applyParse(outcome, silent: silent)
        // This parse supersedes anything scheduled: it ran on the same text.
        cancelScheduledRebuild()
        return result.success
    }
    
    /// Publish one parse result.
    private func applyParse(_ outcome: ParseOutcome, silent: Bool) -> (success: Bool, rewroteText: Bool) {
        rebuildCount += 1
        if let err = outcome.error {
            // A non-empty but invalid document keeps the last good tree, flagged out
            // of date, so the user does not lose their place mid-edit.
            self.parseError = err
            if !silent {
                showError("JSON error: Invalid JSON variable\n\n\(err.message) at line \(err.line), col \(err.column)")
            }
            return (false, false)
        }
        
        if outcome.isEmptyDocument {
            // An empty document has no interpretation left to preserve, so the stale
            // tree, selection, search results and error all go.
            discardTree()
            if !silent {
                showError("JSON error: Please enter JSON code in the Text tab first.")
            }
            return (false, false)
        }
        
        guard let parsed = outcome.value, let root = outcome.root, let restored = outcome.restored else {
            return (false, false)
        }
        
        // Tree node IDs are JSON paths, so they remain stable across rebuilds.
        // Keep the current navigation state and restore the parts that still exist.
        // A node that is gone from the new tree simply was not resolved by the parse.
        let previousSelectionID = selectedNode?.id
        let previousSelectionPath = selectedNode?.path
        let previousExpandedIds = expandedNodeIds
        let previousExpandedLeafIds = expandedLeafNodeIds
        
        var rewroteText = false
        if let rewritten = outcome.rewrittenText, rewritten != rawText {
            setRawTextFromModel(rewritten)
            rewroteText = true
        }
        
        self.treeVersion += 1
        self.jsonValue = parsed
        self.rootNode = root
        
        self.selectedNode = previousSelectionID.flatMap { restored.byID[$0] }
            ?? previousSelectionPath.flatMap { restored.byPath[$0] }
            ?? root
        self.expandedNodeIds = previousExpandedIds.filter { restored.byID[$0]?.isContainer == true }
        self.expandedNodeIds.insert(root.id)
        self.expandedLeafNodeIds = previousExpandedLeafIds.filter { restored.byID[$0]?.isLeaf == true }
        self.updateVisibleRows()
        self.updateSelectedNodeProperties()
        self.parseError = nil
        self.isDirty = false
        return (true, rewroteText)
    }
    
    /// Drop the tree and everything derived from it.
    public func discardTree() {
        self.jsonValue = nil
        self.rootNode = nil
        self.selectedNode = nil
        self.parseError = nil
        self.expandedNodeIds = []
        self.expandedLeafNodeIds = []
        self.visibleTreeRows = []
        clearSearch()
        // Bump the version so rows from the discarded tree can never be reused.
        self.treeVersion += 1
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
        // The model is authoritative here, so any pending editor edit is superseded
        // and the empty document is not re-parsed a moment later.
        lastEditorSelectedRange = NSRange(location: 0, length: 0)
        lastEditorScrollOrigin = .zero
        setRawTextFromModel("")
        cancelScheduledRebuild()
        parseSerial += 1
        // A file still being read must not land on top of the cleared document.
        fileLoadGeneration += 1
        discardTree()
        isDirty = false
    }
    
    // MARK: - Text Transformations (Middle Tab)
    /// Assign model-owned text, superseding any editor edit that has not been
    /// synced yet. The editor picks the new text up from the model.
    private func setRawTextFromModel(_ text: String) {
        pendingTextSync = false
        editorSuppliedMetrics = nil
        modelTextRevision += 1
        rawText = text
    }
    
    public func beautifyText() {
        syncRawTextIfNeeded()
        let trimmed = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        do {
            var val: JSONValue
            if let direct = try? JSONParser.parse(trimmed) {
                val = direct
            } else if let pythonVal = try? PythonLiteralParser.parse(trimmed) {
                val = pythonVal
            } else {
                val = try JSONParser.parse(trimmed)
            }
            if settings.autoUnwrapStringified, case .string(let s) = val {
                let sTrim = s.trimmingCharacters(in: .whitespacesAndNewlines)
                if let unwrap = try? JSONParser.parse(sTrim) {
                    val = unwrap
                } else if let unwrapPython = try? PythonLiteralParser.parse(sTrim) {
                    val = unwrapPython
                }
            }
            setRawTextFromModel(val.format(
                indentSpaces: settings.indentSpaces,
                sortKeys: settings.sortKeysAlphabetically,
                escapeSlashes: settings.escapeSlashesInStringify
            ))
            self.parseAndBuildTree(silent: true)
            triggerCopyFeedback("Formatted JSON")
        } catch {
            showError("Cannot format: Invalid JSON or Python Object (\(error.localizedDescription))")
        }
    }
    
    public func minifyText() {
        syncRawTextIfNeeded()
        let trimmed = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        do {
            let val = try JSONParser.parse(trimmed)
            setRawTextFromModel(val.minify(escapeSlashes: settings.escapeSlashesInStringify))
            self.parseAndBuildTree(silent: true)
            triggerCopyFeedback("Minified JSON")
        } catch {
            showError("Cannot minify: Invalid JSON (\(error.localizedDescription))")
        }
    }
    
    public func stringifyText() {
        syncRawTextIfNeeded()
        let trimmed = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        
        if let val = try? JSONParser.parse(trimmed) {
            setRawTextFromModel(val.stringify(escapeSlashes: settings.escapeSlashesInStringify))
        } else {
            setRawTextFromModel(JSONValue.quoteAndEscapeString(rawText, escapeSlashes: settings.escapeSlashesInStringify))
        }
        self.parseAndBuildTree(silent: true)
        triggerCopyFeedback("Stringified JSON")
    }
    
    public func unescapeText() {
        syncRawTextIfNeeded()
        let unescaped = JSONValue.unescapeStringifiedJSON(rawText)
        setRawTextFromModel(unescaped)
        self.parseAndBuildTree(silent: true)
        triggerCopyFeedback("Unescaped JSON")
    }
    
    public func convertJsonToPython() {
        syncRawTextIfNeeded()
        let trimmed = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        do {
            var val: JSONValue
            if let direct = try? JSONParser.parse(trimmed) {
                val = direct
            } else if let pythonVal = try? PythonLiteralParser.parse(trimmed) {
                val = pythonVal
            } else {
                val = try JSONParser.parse(trimmed)
            }
            let pythonText = val.toPythonObject(indentSpaces: settings.indentSpaces < 0 ? 4 : settings.indentSpaces)
            setRawTextFromModel(pythonText)
            triggerCopyFeedback("Converted JSON to Python!")
        } catch {
            showError("Cannot convert: Invalid JSON (\(error.localizedDescription))")
        }
    }
    
    public func convertPythonToJson() {
        syncRawTextIfNeeded()
        let trimmed = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        do {
            let val = try PythonLiteralParser.parse(trimmed)
            setRawTextFromModel(val.format(indentSpaces: settings.indentSpaces, sortKeys: settings.sortKeysAlphabetically))
            self.parseAndBuildTree(silent: true)
            triggerCopyFeedback("Converted Python to JSON!")
        } catch {
            showError("Cannot convert: Invalid Python dictionary syntax (\(error.localizedDescription))")
        }
    }
    
    // MARK: - Clipboard Operations
    public func copyText() {
        syncRawTextIfNeeded()
        copyToClipboard(rawText)
        triggerCopyFeedback("Copied Text!")
    }
    
    public func copyBeautified() {
        syncRawTextIfNeeded()
        let trimmed = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        if let val = jsonValue ?? (try? JSONParser.parse(trimmed)) {
            let text = val.format(
                indentSpaces: settings.indentSpaces,
                sortKeys: settings.sortKeysAlphabetically,
                escapeSlashes: false
            )
            copyToClipboard(text)
            triggerCopyFeedback("Copied Beautified JSON!")
        } else {
            copyToClipboard(rawText)
            triggerCopyFeedback("Copied Text!")
        }
    }
    
    public func copyMinified() {
        syncRawTextIfNeeded()
        let trimmed = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        if let val = jsonValue ?? (try? JSONParser.parse(trimmed)) {
            let text = val.minify(escapeSlashes: false)
            copyToClipboard(text)
            triggerCopyFeedback("Copied Minified JSON!")
        } else {
            copyToClipboard(rawText)
            triggerCopyFeedback("Copied Text!")
        }
    }
    
    public func copyStringified() {
        syncRawTextIfNeeded()
        let trimmed = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        if let val = jsonValue ?? (try? JSONParser.parse(trimmed)) {
            let text = val.stringify(escapeSlashes: settings.escapeSlashesInStringify)
            copyToClipboard(text)
            triggerCopyFeedback("Copied Stringified JSON!")
        } else {
            let text = JSONValue.quoteAndEscapeString(rawText, escapeSlashes: settings.escapeSlashesInStringify)
            copyToClipboard(text)
            triggerCopyFeedback("Copied Stringified Text!")
        }
    }
    
    public func copyPythonObject() {
        syncRawTextIfNeeded()
        let trimmed = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        if let val = jsonValue ?? (try? JSONParser.parse(trimmed)) ?? (try? PythonLiteralParser.parse(trimmed)) {
            let text = val.toPythonObject(indentSpaces: settings.indentSpaces < 0 ? 4 : settings.indentSpaces)
            copyToClipboard(text)
            triggerCopyFeedback("Copied Python Dictionary!")
        } else {
            copyToClipboard(rawText)
            triggerCopyFeedback("Copied Text!")
        }
    }
    
    private func copyToClipboard(_ str: String) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(str, forType: .string)
    }
    
    public func triggerCopyFeedback(_ message: String = "Copied!") {
        self.copiedToastMessage = message
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.8) { [weak self] in
            if self?.copiedToastMessage == message {
                self?.copiedToastMessage = nil
            }
        }
    }
    
    public func pasteText() {
        let pasteboard = NSPasteboard.general
        if let string = pasteboard.string(forType: .string) {
            let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
            // Auto-convert Python dictionary to JSON on paste if not already standard JSON
            if (try? JSONParser.parse(trimmed)) == nil, let pythonVal = try? PythonLiteralParser.parse(trimmed) {
                setRawTextFromModel(pythonVal.format(indentSpaces: settings.indentSpaces, sortKeys: settings.sortKeysAlphabetically))
                parseAndBuildTree(silent: true)
                triggerCopyFeedback("Converted Python Dictionary to JSON!")
                return
            }
            setRawTextFromModel(string)
            parseAndBuildTree(silent: true)
        }
    }
    
    // MARK: - Search
    public func clearSearch() {
        let queryWasAlreadyEmpty = searchQuery.isEmpty
        cancelSearchTask()
        searchQuery = ""
        searchResults = []
        searchResultIds = []
        searchStatus = ""
        lastExecutedSearchQuery = ""
        currentSearchIndex = 0
        if queryWasAlreadyEmpty {
            treeScrollToTopRequest += 1
        }
    }

    private func cancelSearchTask() {
        searchTask?.cancel()
        searchTask = nil
        searchGeneration += 1
    }

    public func searchStart(reverse: Bool = false) {
        syncRawTextIfNeeded()
        let query = searchQuery.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else {
            clearSearch()
            return
        }

        cancelSearchTask()
        let generation = searchGeneration

        lastExecutedSearchQuery = query
        
        guard let root = rootNode else {
            // Try building tree first
            if parseAndBuildTree(silent: true), let r = rootNode {
                performSearch(on: r, query: query, generation: generation, reverse: reverse)
            } else {
                searchStatus = "Phrase not found!"
            }
            return
        }
        
        performSearch(on: root, query: query, generation: generation, reverse: reverse)
    }

    private func performSearch(on root: JSONNode, query: String, generation: Int, reverse: Bool) {
        searchStatus = "Searching…"
        searchTask = Task.detached(priority: .userInitiated) { [weak self] in
            let matches = root.searchMatches(query: query)
            guard !Task.isCancelled else { return }
            await self?.finishSearch(matches, query: query, generation: generation, reverse: reverse)
        }
    }

    private func finishSearch(_ matches: [JSONNode], query: String, generation: Int, reverse: Bool) {
        guard generation == searchGeneration,
              query == searchQuery.trimmingCharacters(in: .whitespacesAndNewlines) else { return }
        searchTask = nil
        self.searchResults = matches
        self.searchResultIds = Set(matches.map { $0.id })

        if matches.isEmpty {
            self.searchStatus = "Phrase not found!"
        } else {
            self.currentSearchIndex = reverse ? matches.count - 1 : 0
            selectMatch(at: self.currentSearchIndex)
        }
    }
    
    public func searchNext() {
        let query = searchQuery.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return }
        
        if query != lastExecutedSearchQuery || searchResults.isEmpty {
            if searchTask != nil { return }
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
            if searchTask != nil { return }
            searchStart(reverse: true)
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
        selectAndReveal(node: target)

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
        // Signal after expanding ancestors and rebuilding rows so the view can
        // scroll once the lazy row is part of the visible tree.
        self.treeNavigationRequest += 1
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
    private var fileLoadGeneration = 0
    
    /// Read on a background queue and rebuild off the main thread for a large file.
    ///
    /// Both halves used to run on the main actor: reading a multi-megabyte file, and
    /// then parsing it. Opening a 24 MB file froze the app for ~0.48 s while the open
    /// panel was still dismissing.
    public func openFile(url: URL) {
        // Asking for a file supersedes whatever was pending: an editor edit that has
        // not been synced, a scheduled rebuild, and a rebuild already in flight.
        fileLoadGeneration += 1
        let generation = fileLoadGeneration
        pendingTextSync = false
        editorSuppliedMetrics = nil
        rebuildTask?.cancel()
        cancelScheduledRebuild()
        
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            var failure: String?
            var text: String?
            do {
                let data = try Data(contentsOf: url)
                if let decoded = String(data: data, encoding: .utf8) {
                    text = decoded
                } else {
                    failure = "Failed to open file: \(url.lastPathComponent) is not valid UTF-8 text."
                }
            } catch {
                failure = "Failed to open file: \(error.localizedDescription)"
            }
            
            Task { @MainActor [weak self] in
                // A newer open, or a clear, happened while this file was being read.
                guard let self = self, generation == self.fileLoadGeneration else { return }
                if let text = text {
                    self.setRawTextFromModel(text)
                    // Parsing is deferred to a background task for a large document, so
                    // the tree appears a moment later instead of freezing the app here.
                    self.startRebuild(silent: true)
                } else if let failure = failure {
                    self.showError(failure)
                }
            }
        }
    }
    
    public func saveToFile(url: URL) {
        syncRawTextIfNeeded()
        do {
            try rawText.write(to: url, atomically: true, encoding: .utf8)
            isDirty = false
        } catch {
            showError("Failed to save file: \(error.localizedDescription)")
        }
    }
}
