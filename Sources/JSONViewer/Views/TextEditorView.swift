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
            // Text Toolbar matching jsonviewer.stack.hu
            HStack(spacing: 6) {
                Button(action: {
                    model.pasteText()
                }) {
                    Label("Paste", systemImage: "doc.on.clipboard")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                
                Button(action: {
                    model.copyText()
                }) {
                    Label("Copy", systemImage: "doc.on.doc")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                
                Divider()
                    .frame(height: 16)
                
                Button(action: {
                    model.formatJSON()
                }) {
                    Label("Format", systemImage: "text.alignleft")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                
                Button(action: {
                    model.removeWhitespace()
                }) {
                    Label("Remove white space", systemImage: "arrow.right.to.line.compact")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                
                Divider()
                    .frame(height: 16)
                
                Button(action: {
                    model.clearText()
                }) {
                    Label("Clear", systemImage: "trash")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                
                Divider()
                    .frame(height: 16)
                
                Button(action: {
                    model.isLoadURLSheetPresented = true
                }) {
                    Label("Load JSON data", systemImage: "globe")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                
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
                    model.isAboutSheetPresented = true
                }) {
                    Text("About")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(Color(nsColor: .controlBackgroundColor))
            
            Divider()
            
            // Editor Area with Line Numbers
            NativeCodeEditor(text: $model.rawText, fontSize: model.fontSize)
                .overlay(alignment: .topLeading) {
                    if model.rawText.isEmpty {
                        Text("Paste the JSON code here (your code is not saved anywhere)")
                            .font(.system(size: model.fontSize, design: .monospaced))
                            .foregroundColor(.secondary.opacity(0.6))
                            .padding(.leading, 52)
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
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.json, .plainText]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        if panel.runModal() == .OK, let url = panel.url {
            model.openFile(url: url)
        }
    }
    
    private func promptSaveFile() {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.json]
        panel.nameFieldStringValue = "document.json"
        if panel.runModal() == .OK, let url = panel.url {
            model.saveToFile(url: url)
        }
    }
}

// MARK: - AppKit Native Editor with Line Number Gutter
struct NativeCodeEditor: NSViewRepresentable {
    @Binding var text: String
    var fontSize: CGFloat = 13
    
    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }
    
    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = true
        scrollView.borderType = .noBorder
        
        let textView = NSTextView()
        textView.autoresizingMask = [.width]
        textView.font = NSFont.monospacedSystemFont(ofSize: fontSize, weight: .regular)
        textView.backgroundColor = NSColor.textBackgroundColor
        textView.textColor = NSColor.textColor
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticTextReplacementEnabled = false
        textView.isAutomaticSpellingCorrectionEnabled = false
        textView.allowsUndo = true
        textView.isRichText = false
        textView.string = text
        textView.delegate = context.coordinator
        
        // Padding for gutter
        textView.textContainer?.lineFragmentPadding = 8
        
        scrollView.documentView = textView
        context.coordinator.textView = textView
        
        return scrollView
    }
    
    func updateNSView(_ nsView: NSScrollView, context: Context) {
        guard let textView = nsView.documentView as? NSTextView else { return }
        if textView.font?.pointSize != fontSize {
            textView.font = NSFont.monospacedSystemFont(ofSize: fontSize, weight: .regular)
        }
        if textView.string != text {
            let selectedRanges = textView.selectedRanges
            textView.undoManager?.removeAllActions()
            textView.string = text
            textView.selectedRanges = selectedRanges
        }
    }
    
    class Coordinator: NSObject, NSTextViewDelegate {
        var parent: NativeCodeEditor
        weak var textView: NSTextView?
        
        init(_ parent: NativeCodeEditor) {
            self.parent = parent
        }
        
        func textDidChange(_ notification: Notification) {
            guard let tv = notification.object as? NSTextView else { return }
            if parent.text != tv.string {
                parent.text = tv.string
            }
        }
    }
}
