import SwiftUI
import JSONViewerCore

public struct LoadURLSheet: View {
    @ObservedObject var model: JSONDocumentModel
    @Environment(\.dismiss) private var dismiss
    @FocusState private var isFieldFocused: Bool
    
    public init(model: JSONDocumentModel) {
        self.model = model
    }
    
    public var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Label("Load JSON Data from URL", systemImage: "globe")
                    .font(.headline)
                Spacer()
                Button(action: { dismiss() }) {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundColor(.secondary)
                }
                .buttonStyle(.plain)
            }
            
            Text("Enter a public HTTP or HTTPS URL returning JSON:")
                .font(.subheadline)
                .foregroundColor(.secondary)
            
            HStack(spacing: 8) {
                Text("URL:")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundColor(.secondary)
                
                TextField("https://", text: $model.urlInput)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(size: 13, design: .monospaced))
                    .focused($isFieldFocused)
                    .onSubmit {
                        Task {
                            await model.loadRemoteJSON(from: model.urlInput)
                        }
                    }
            }
            
            if let err = model.urlErrorMessage {
                HStack {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundColor(.red)
                    Text(err)
                        .font(.caption)
                        .foregroundColor(.red)
                }
            }
            
            // Sample URLs for quick testing
            VStack(alignment: .leading, spacing: 4) {
                Text("Quick test samples:")
                    .font(.caption)
                    .foregroundColor(.secondary)
                
                HStack {
                    sampleButton(title: "GitHub API Status", url: "https://api.github.com")
                    sampleButton(title: "JSONPlaceholder Todo", url: "https://jsonplaceholder.typicode.com/todos/1")
                }
            }
            
            HStack {
                Spacer()
                
                Button("Cancel") {
                    dismiss()
                }
                .keyboardShortcut(.cancelAction)
                
                Button(action: {
                    Task {
                        await model.loadRemoteJSON(from: model.urlInput)
                    }
                }) {
                    if model.isURLSubmitting {
                        ProgressView()
                            .controlSize(.small)
                            .padding(.horizontal, 8)
                    } else {
                        Text("Load JSON data!")
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(model.isURLSubmitting || model.urlInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(20)
        .frame(width: 480)
        .onAppear {
            isFieldFocused = true
        }
    }
    
    private func sampleButton(title: String, url: String) -> some View {
        Button(action: {
            model.urlInput = url
        }) {
            Text(title)
                .font(.caption2)
        }
        .buttonStyle(.bordered)
        .controlSize(.mini)
    }
}
