import SwiftUI
import JSONViewerCore

public struct SearchToolbar: View {
    @ObservedObject var model: JSONDocumentModel
    @FocusState private var isSearchFieldFocused: Bool
    
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
            
            HStack {
                Image(systemName: "magnifyingglass")
                    .foregroundColor(.secondary)
                    .font(.system(size: 11))
                
                TextField("Search keys, values, paths...", text: $model.searchQuery)
                    .textFieldStyle(.plain)
                    .font(.system(size: 12, design: .monospaced))
                    .focused($isSearchFieldFocused)
                    .onSubmit {
                        model.searchStart()
                    }
                
                if !model.searchQuery.isEmpty {
                    Button(action: {
                        model.searchQuery = ""
                        model.searchResults = []
                        model.searchStatus = ""
                    }) {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundColor(.secondary)
                            .font(.system(size: 11))
                    }
                    .buttonStyle(.plain)
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
                model.searchStart()
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.small)
            .keyboardShortcut(.defaultAction)
            
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
