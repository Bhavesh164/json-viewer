import SwiftUI
import JSONViewerCore

public struct SearchToolbar: View {
    @ObservedObject var model: JSONDocumentModel
    @State private var isSearchFieldFocused: Bool = false
    
    public init(model: JSONDocumentModel) {
        self.model = model
    }
    
    public var body: some View {
        HStack(spacing: 8) {
            Text("Search:")
                .font(.system(size: 12, weight: .semibold, design: .default))
                .foregroundColor(.secondary)
                .lineLimit(1)
                .fixedSize(horizontal: true, vertical: false)
            
            HStack(spacing: 6) {
                Image(systemName: "magnifyingglass")
                    .foregroundColor(.secondary)
                    .font(.system(size: 11))
                
                SearchInputTextField(
                    text: $model.searchQuery,
                    isFocused: $isSearchFieldFocused,
                    placeholder: "Search keys, values, paths...",
                    onEnter: {
                        model.searchSubmit(reverse: false)
                    },
                    onShiftEnter: {
                        model.searchSubmit(reverse: true)
                    },
                    onEscape: {
                        model.clearSearch()
                    }
                )
                .frame(height: 20)
                
                if !model.searchQuery.isEmpty {
                    Button(action: {
                        model.clearSearch()
                    }) {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundColor(.secondary)
                            .font(.system(size: 11))
                    }
                    .buttonStyle(.plain)
                    .help("Clear search")
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .fill(Color(nsColor: .textBackgroundColor))
                    .overlay(
                        RoundedRectangle(cornerRadius: 6)
                            .stroke(isSearchFieldFocused ? Color.accentColor : Color.gray.opacity(0.3), lineWidth: 1)
                    )
            )
            .frame(minWidth: 180, maxWidth: 300)
            
            Button("GO!") {
                model.searchSubmit(reverse: false)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.small)
            .help("Execute search or go to next match (Enter)")
            
            if !model.searchStatus.isEmpty {
                Text(model.searchStatus)
                    .font(.system(size: 11, weight: .medium))
                    .foregroundColor(model.searchStatus == "Phrase not found!" ? .red : .primary)
                    .lineLimit(1)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(
                        RoundedRectangle(cornerRadius: 4)
                            .fill(model.searchStatus == "Phrase not found!" ? Color.red.opacity(0.1) : Color.accentColor.opacity(0.1))
                    )
            }
            
            Spacer()
            
            HStack(spacing: 4) {
                Button(action: {
                    model.searchPrevious()
                }) {
                    HStack(spacing: 3) {
                        Image(systemName: "arrow.up")
                        Text("Previous")
                    }
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .disabled(model.searchResults.isEmpty)
                .help("Previous match (Shift+Enter or Cmd+Shift+G)")
                .keyboardShortcut("g", modifiers: [.command, .shift])
                
                Button(action: {
                    model.searchNext()
                }) {
                    HStack(spacing: 3) {
                        Image(systemName: "arrow.down")
                        Text("Next")
                    }
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .disabled(model.searchResults.isEmpty)
                .help("Next match (Enter or Cmd+G)")
                .keyboardShortcut("g", modifiers: [.command])
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(Color(nsColor: .windowBackgroundColor))
        .overlay(
            Rectangle()
                .frame(height: 1)
                .foregroundColor(Color(nsColor: .separatorColor)),
            alignment: .top
        )
    }
}

// MARK: - Native AppKit Search Input Text Field
public struct SearchInputTextField: NSViewRepresentable {
    @Binding var text: String
    @Binding var isFocused: Bool
    var placeholder: String
    var onEnter: () -> Void
    var onShiftEnter: () -> Void
    var onEscape: () -> Void
    
    public init(
        text: Binding<String>,
        isFocused: Binding<Bool>,
        placeholder: String = "Search keys, values, paths...",
        onEnter: @escaping () -> Void,
        onShiftEnter: @escaping () -> Void,
        onEscape: @escaping () -> Void
    ) {
        self._text = text
        self._isFocused = isFocused
        self.placeholder = placeholder
        self.onEnter = onEnter
        self.onShiftEnter = onShiftEnter
        self.onEscape = onEscape
    }
    
    public func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }
    
    public func makeNSView(context: Context) -> NSTextField {
        let textField = NSTextField()
        textField.isBordered = false
        textField.drawsBackground = false
        textField.focusRingType = .none
        textField.font = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
        textField.textColor = NSColor.textColor
        textField.placeholderString = placeholder
        textField.maximumNumberOfLines = 1
        textField.cell?.wraps = false
        textField.cell?.isScrollable = true
        textField.cell?.usesSingleLineMode = true
        textField.stringValue = text
        textField.delegate = context.coordinator
        context.coordinator.textField = textField
        return textField
    }
    
    public func updateNSView(_ nsView: NSTextField, context: Context) {
        if nsView.stringValue != text {
            nsView.stringValue = text
        }
        if nsView.placeholderString != placeholder {
            nsView.placeholderString = placeholder
        }
    }
    
    public static func dismantleNSView(_ nsView: NSTextField, coordinator: Coordinator) {
        coordinator.cleanup()
    }
    
    public class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: SearchInputTextField
        weak var textField: NSTextField?
        private var keyMonitor: Any?
        
        init(_ parent: SearchInputTextField) {
            self.parent = parent
            super.init()
            setupKeyMonitor()
        }
        
        private func setupKeyMonitor() {
            keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
                guard let self = self, let tf = self.textField else { return event }
                let isEditing = (tf.currentEditor() != nil) || (tf.window?.firstResponder === tf)
                guard isEditing else { return event }
                
                if event.keyCode == 36 || event.keyCode == 76 { // Return / Keypad Enter
                    let isShift = event.modifierFlags.contains(.shift)
                    DispatchQueue.main.async {
                        if isShift {
                            self.parent.onShiftEnter()
                        } else {
                            self.parent.onEnter()
                        }
                    }
                    return nil
                } else if event.keyCode == 53 { // Escape
                    DispatchQueue.main.async {
                        self.parent.onEscape()
                    }
                    return nil
                }
                return event
            }
        }
        
        func cleanup() {
            if let monitor = keyMonitor {
                NSEvent.removeMonitor(monitor)
                keyMonitor = nil
            }
        }
        
        deinit {
            cleanup()
        }
        
        public func controlTextDidChange(_ obj: Notification) {
            guard let tf = obj.object as? NSTextField else { return }
            parent.text = tf.stringValue
        }
        
        public func controlTextDidBeginEditing(_ obj: Notification) {
            DispatchQueue.main.async {
                self.parent.isFocused = true
            }
        }
        
        public func controlTextDidEndEditing(_ obj: Notification) {
            DispatchQueue.main.async {
                self.parent.isFocused = false
            }
        }
        
        public func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                let isShift = NSEvent.modifierFlags.contains(.shift)
                if isShift {
                    parent.onShiftEnter()
                } else {
                    parent.onEnter()
                }
                return true
            } else if commandSelector == #selector(NSResponder.cancelOperation(_:)) {
                parent.onEscape()
                return true
            }
            return false
        }
    }
}
