import SwiftUI
import AppKit
import JSONViewerCore

public struct TextEditorView: View {
    @ObservedObject var model: JSONDocumentModel
    
    public init(model: JSONDocumentModel) {
        self.model = model
    }
    
    public var body: some View {
        VStack(spacing: 0) {
            // Text Toolbar matching jsonviewer.stack.hu with Format, Minify, Stringify, Unescape
            HStack(spacing: 6) {
                Button(action: {
                    model.pasteText()
                }) {
                    Label("Paste", systemImage: "doc.on.clipboard")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                
                // Copy split-menu with 4 formats
                Menu {
                    Button(action: { model.copyBeautified() }) {
                        Label("Copy Beautified JSON", systemImage: "text.alignleft")
                    }
                    Button(action: { model.copyMinified() }) {
                        Label("Copy Minified JSON", systemImage: "arrow.right.to.line.compact")
                    }
                    Button(action: { model.copyStringified() }) {
                        Label("Copy as Stringified JSON", systemImage: "quote.bubble")
                    }
                    Button(action: { model.copyPythonObject() }) {
                        Label("Copy as Python Dictionary", systemImage: "curlybraces.square")
                    }
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: model.copiedToastMessage != nil ? "checkmark" : "doc.on.doc")
                        Text(model.copiedToastMessage ?? "Copy")
                    }
                } primaryAction: {
                    model.copyText()
                }
                .menuStyle(.borderedButton)
                .controlSize(.small)
                .help("Click to copy text, or open dropdown to copy formatted, minified, stringified, or Python dictionary")
                
                Button(action: {
                    model.clearText()
                }) {
                    Label("Clear", systemImage: "trash")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                
                Divider()
                    .frame(height: 16)
                
                // Format & Transform options in middle tab
                Button(action: {
                    model.beautifyText()
                }) {
                    Label("Format", systemImage: "text.alignleft")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("Format JSON with configured indentation")
                
                Button(action: {
                    model.minifyText()
                }) {
                    Label("Minify", systemImage: "arrow.right.to.line.compact")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("Minify JSON into single line")
                
                Button(action: {
                    model.stringifyText()
                }) {
                    Label("Stringify", systemImage: "quote.bubble")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("Stringify JSON into escaped string literal with slashes (\\/) and quotes (\\\")")
                
                Button(action: {
                    model.unescapeText()
                }) {
                    Label("Unescape", systemImage: "character.textbox")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("Unescape stringified JSON or escaped slashes/characters back to formatted JSON")
                
                Divider()
                    .frame(height: 16)
                
                Button(action: {
                    model.convertJsonToPython()
                }) {
                    Label("JSON → Python", systemImage: "curlybraces.square")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("Convert JSON in editor directly into Python dictionary format")
                
                Button(action: {
                    model.convertPythonToJson()
                }) {
                    Label("Python → JSON", systemImage: "arrow.triangle.2.circlepath")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("Convert Python dictionary in editor directly into valid JSON")
                
                Spacer()
                
                Button(action: {
                    promptOpenFile()
                }) {
                    Image(systemName: "folder")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("Open JSON File (Cmd+O)")
                
                Button(action: {
                    promptSaveFile()
                }) {
                    Image(systemName: "square.and.arrow.down")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("Save JSON File (Cmd+S)")
                
                Divider()
                    .frame(height: 16)
                
                Button(action: { model.zoomOut() }) {
                    Image(systemName: "minus.magnifyingglass")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("Zoom Out (Cmd -)")
                
                Button(action: { model.zoomIn() }) {
                    Image(systemName: "plus.magnifyingglass")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("Zoom In (Cmd +)")
                
                Button(action: {
                    model.isShortcutsSheetPresented = true
                }) {
                    Image(systemName: "questionmark.circle")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("Keyboard Shortcuts (?)")
                
                Button(action: {
                    model.isSettingsSheetPresented = true
                }) {
                    Image(systemName: "gearshape")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("Settings (Cmd+,)")
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(Color(nsColor: .controlBackgroundColor))
            
            Divider()
            
            // Editor Area with Native Text Editor
            NativeCodeEditor(text: $model.rawText, fontSize: model.fontSize, wrapLines: model.settings.wrapLines)
                .overlay(alignment: .topLeading) {
                    if model.rawText.isEmpty {
                        Text("Paste the JSON code here (your code is not saved anywhere)")
                            .font(.system(size: model.fontSize, design: .monospaced))
                            .foregroundColor(.secondary.opacity(0.6))
                            .padding(.leading, 12)
                            .padding(.top, 10)
                            .allowsHitTesting(false)
                    }
                }
            
            Divider()
            
            // Bottom Status Bar
            HStack {
                if let err = model.parseError {
                    HStack(spacing: 4) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .foregroundColor(.red)
                        Text("Error: \(err.message) (Line \(err.line), Col \(err.column))")
                            .font(.system(size: 11, weight: .medium))
                            .foregroundColor(.red)
                    }
                } else if !model.rawText.isEmpty {
                    HStack(spacing: 4) {
                        Image(systemName: "checkmark.circle.fill")
                            .foregroundColor(.green)
                        Text("Valid JSON")
                            .font(.system(size: 11))
                            .foregroundColor(.secondary)
                    }
                } else {
                    Text("Ready")
                        .font(.system(size: 11))
                        .foregroundColor(.secondary)
                }
                
                Spacer()
                
                Text("\(model.lineCount) lines, \(model.characterCount) characters")
                    .font(.system(size: 11))
                    .foregroundColor(.secondary)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 4)
            .background(Color(nsColor: .windowBackgroundColor))
        }
    }
    
    private func promptOpenFile() {
        JSONViewer.promptOpenFile(model: model)
    }
    
    private func promptSaveFile() {
        JSONViewer.promptSaveFile(model: model)
    }
}

// Custom NSTextView that automatically converts pasted Python dictionaries/literals to valid JSON
final class EditorTextView: NSTextView {
    override init(frame frameRect: NSRect, textContainer: NSTextContainer?) {
        super.init(frame: frameRect, textContainer: textContainer)
    }
    
    required init?(coder: NSCoder) {
        super.init(coder: coder)
    }
    
    convenience override init(frame frameRect: NSRect) {
        self.init(frame: frameRect, textContainer: nil)
    }
    
    convenience init() {
        self.init(frame: .zero, textContainer: nil)
    }
    
    override func paste(_ sender: Any?) {
        if let pbString = NSPasteboard.general.string(forType: .string) {
            let trimmed = pbString.trimmingCharacters(in: .whitespacesAndNewlines)
            if (try? JSONParser.parse(trimmed)) == nil, let pythonVal = try? PythonLiteralParser.parse(trimmed) {
                let formattedJSON = pythonVal.format(indentSpaces: 2, sortKeys: false)
                self.insertText(formattedJSON, replacementRange: self.selectedRange())
                return
            }
        }
        super.paste(sender)
    }
}

// MARK: - AppKit High-Performance Native Code Editor
struct NativeCodeEditor: NSViewRepresentable {
    @Binding var text: String
    var fontSize: CGFloat = 13
    var wrapLines: Bool = false
    
    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }
    
    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = true
        scrollView.borderType = .noBorder
        scrollView.autohidesScrollers = true
        
        // Explicitly build the TextKit 1 stack with NSTextStorage and NSLayoutManager.
        // This is the proven, highest-performance configuration for large documents in AppKit,
        // ensuring non-contiguous layout and idle background layout actually take effect.
        let textStorage = NSTextStorage(string: text)
        let layoutManager = NSLayoutManager()
        layoutManager.allowsNonContiguousLayout = true
        layoutManager.backgroundLayoutEnabled = true
        textStorage.addLayoutManager(layoutManager)
        
        let contentSize = scrollView.contentSize
        let textContainer = NSTextContainer(containerSize: NSSize(
            width: wrapLines ? contentSize.width : CGFloat.greatestFiniteMagnitude,
            height: CGFloat.greatestFiniteMagnitude
        ))
        textContainer.widthTracksTextView = wrapLines
        textContainer.lineFragmentPadding = 8
        layoutManager.addTextContainer(textContainer)
        
        let textView = EditorTextView(frame: NSRect(origin: .zero, size: contentSize), textContainer: textContainer)
        textView.minSize = NSSize(width: 0.0, height: contentSize.height)
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = !wrapLines
        textView.autoresizingMask = wrapLines ? [.width] : []
        
        let font = NSFont.monospacedSystemFont(ofSize: fontSize, weight: .regular)
        textView.font = font
        textView.backgroundColor = NSColor.textBackgroundColor
        textView.textColor = NSColor.textColor
        textView.typingAttributes = [
            .font: font,
            .foregroundColor: NSColor.textColor
        ]
        
        let fullRange = NSRange(location: 0, length: textStorage.length)
        if fullRange.length > 0 {
            textStorage.addAttribute(.font, value: font, range: fullRange)
            textStorage.addAttribute(.foregroundColor, value: NSColor.textColor, range: fullRange)
        }
        
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticTextReplacementEnabled = false
        textView.isAutomaticSpellingCorrectionEnabled = false
        textView.isContinuousSpellCheckingEnabled = false
        textView.isGrammarCheckingEnabled = false
        textView.isAutomaticLinkDetectionEnabled = false
        textView.isAutomaticDataDetectionEnabled = false
        textView.isAutomaticTextCompletionEnabled = false
        textView.smartInsertDeleteEnabled = false
        textView.allowsUndo = true
        textView.isRichText = false
        textView.allowsDocumentBackgroundColorChange = false
        
        textView.delegate = context.coordinator
        context.coordinator.textView = textView
        
        scrollView.documentView = textView
        return scrollView
    }
    
    func updateNSView(_ nsView: NSScrollView, context: Context) {
        context.coordinator.parent = self
        guard let textView = nsView.documentView as? EditorTextView,
              let textStorage = textView.textStorage else { return }
        
        // 1. Update font size if changed
        if textView.font?.pointSize != fontSize {
            let newFont = NSFont.monospacedSystemFont(ofSize: fontSize, weight: .regular)
            textView.font = newFont
            textView.typingAttributes[.font] = newFont
            if textStorage.length > 0 {
                textStorage.addAttribute(.font, value: newFont, range: NSRange(location: 0, length: textStorage.length))
            }
        }
        
        // 2. Update line wrapping if changed
        let isCurrentlyWrapping = textView.textContainer?.widthTracksTextView == true
        if isCurrentlyWrapping != wrapLines {
            textView.isHorizontallyResizable = !wrapLines
            textView.textContainer?.widthTracksTextView = wrapLines
            if wrapLines {
                textView.autoresizingMask = [.width]
                textView.textContainer?.containerSize = NSSize(width: nsView.contentSize.width, height: CGFloat.greatestFiniteMagnitude)
            } else {
                textView.autoresizingMask = []
                textView.textContainer?.containerSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
            }
            textView.needsLayout = true
        }
        
        // 3. Skip update if triggered from this text view's own typing
        if context.coordinator.isUpdatingFromTextView {
            return
        }
        
        // 4. Ultra-fast length check directly on NSTextStorage without copying full strings
        let currentLength = textStorage.length
        let nsNewText = text as NSString
        let newLength = nsNewText.length
        
        if currentLength != newLength || textStorage.string != text {
            let selectedRanges = textView.selectedRanges
            textView.undoManager?.removeAllActions()
            
            // Batch edits in NSTextStorage to avoid expensive repeated layout passes
            textStorage.beginEditing()
            textStorage.replaceCharacters(in: NSRange(location: 0, length: currentLength), with: text)
            
            let updatedRange = NSRange(location: 0, length: textStorage.length)
            if updatedRange.length > 0 {
                let font = textView.font ?? NSFont.monospacedSystemFont(ofSize: fontSize, weight: .regular)
                textStorage.addAttribute(.font, value: font, range: updatedRange)
                textStorage.addAttribute(.foregroundColor, value: NSColor.textColor, range: updatedRange)
            }
            textStorage.endEditing()
            
            // Restore selection clamped to valid textStorage length
            let maxLen = textStorage.length
            let validRanges = selectedRanges.compactMap { val -> NSValue? in
                let r = val.rangeValue
                if r.location <= maxLen {
                    let len = min(r.length, maxLen - r.location)
                    return NSValue(range: NSRange(location: r.location, length: len))
                }
                return nil
            }
            if !validRanges.isEmpty {
                textView.selectedRanges = validRanges
            }
        }
    }
    
    static func dismantleNSView(_ nsView: NSScrollView, coordinator: Coordinator) {
        if let textView = nsView.documentView as? NSTextView {
            textView.delegate = nil
        }
        coordinator.textView = nil
    }
    
    class Coordinator: NSObject, NSTextViewDelegate {
        var parent: NativeCodeEditor
        weak var textView: EditorTextView?
        var isUpdatingFromTextView: Bool = false
        
        init(_ parent: NativeCodeEditor) {
            self.parent = parent
        }
        
        func textDidChange(_ notification: Notification) {
            guard let tv = notification.object as? NSTextView else { return }
            isUpdatingFromTextView = true
            parent.text = tv.string
            // Clear flag asynchronously after SwiftUI finishes this update cycle
            DispatchQueue.main.async { [weak self] in
                self?.isUpdatingFromTextView = false
            }
        }
        
        func textDidEndEditing(_ notification: Notification) {
            guard let tv = notification.object as? NSTextView else { return }
            if parent.text != tv.string {
                parent.text = tv.string
            }
        }
    }
}
