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
        
        let textView = EditorTextView()
        textView.autoresizingMask = [.width]
        textView.font = NSFont.monospacedSystemFont(ofSize: fontSize, weight: .regular)
        textView.backgroundColor = NSColor.textBackgroundColor
        textView.textColor = NSColor.textColor
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticTextReplacementEnabled = false
        textView.isAutomaticSpellingCorrectionEnabled = false
        textView.isContinuousSpellCheckingEnabled = false
        textView.isGrammarCheckingEnabled = false
        textView.isAutomaticLinkDetectionEnabled = false
        textView.isAutomaticDataDetectionEnabled = false
        textView.smartInsertDeleteEnabled = false
        textView.allowsUndo = true
        textView.isRichText = false
        
        if wrapLines {
            textView.isHorizontallyResizable = false
            textView.textContainer?.widthTracksTextView = true
        } else {
            textView.isHorizontallyResizable = true
            textView.textContainer?.widthTracksTextView = false
            textView.textContainer?.containerSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        }
        
        // Critical for performance: only lay out visible text lines instead of calculating millions of glyphs
        textView.layoutManager?.allowsNonContiguousLayout = true
        
        textView.string = text
        textView.delegate = context.coordinator
        
        textView.textContainer?.lineFragmentPadding = 8
        
        scrollView.documentView = textView
        context.coordinator.textView = textView
        
        return scrollView
    }
    
    func updateNSView(_ nsView: NSScrollView, context: Context) {
        context.coordinator.parent = self
        guard let textView = nsView.documentView as? NSTextView else { return }
        
        if textView.font?.pointSize != fontSize {
            textView.font = NSFont.monospacedSystemFont(ofSize: fontSize, weight: .regular)
        }
        
        let isCurrentlyWrapping = textView.textContainer?.widthTracksTextView == true
        if isCurrentlyWrapping != wrapLines {
            textView.isHorizontallyResizable = !wrapLines
            textView.textContainer?.widthTracksTextView = wrapLines
            if !wrapLines {
                textView.textContainer?.containerSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
            }
        }
        
        // If the update was triggered by the user typing directly in this textView, skip immediately!
        // This eliminates redundant string comparisons and avoids resetting the undo manager while typing.
        if context.coordinator.isUpdatingFromTextView {
            return
        }
        
        // Fast integer length check before performing an expensive full-string comparison
        let currentLength = (textView.string as NSString).length
        let newLength = (text as NSString).length
        if currentLength != newLength || textView.string != text {
            let selectedRanges = textView.selectedRanges
            textView.undoManager?.removeAllActions()
            textView.string = text
            textView.selectedRanges = selectedRanges
        }
    }
    
    class Coordinator: NSObject, NSTextViewDelegate {
        var parent: NativeCodeEditor
        weak var textView: NSTextView?
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
    }
}
