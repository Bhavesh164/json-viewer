import SwiftUI

public struct AboutSheet: View {
    @Environment(\.dismiss) private var dismiss
    
    public init() {}
    
    public var body: some View {
        VStack(spacing: 16) {
            Image(nsImage: NSApplication.shared.applicationIconImage)
                .resizable()
                .scaledToFit()
                .frame(width: 72, height: 72)
            
            VStack(spacing: 4) {
                Text("JSON Viewer for macOS")
                    .font(.title2.bold())
                Text("Version 1.0 (Native Swift)")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }
            
            Divider()
                .frame(width: 280)
            
            VStack(alignment: .leading, spacing: 8) {
                Text("A high-performance macOS native application reimagining the classic web JSON Viewer (jsonviewer.stack.hu) originally authored by Gabor Turi.")
                    .font(.system(size: 12))
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
                
                VStack(alignment: .leading, spacing: 4) {
                    bulletPoint(icon: "checkmark.circle.fill", text: "Interactive collapsible tree with typed badge colors")
                    bulletPoint(icon: "checkmark.circle.fill", text: "Property Grid inspector for direct node attributes")
                    bulletPoint(icon: "checkmark.circle.fill", text: "2-space pretty formatting & whitespace minifier")
                    bulletPoint(icon: "checkmark.circle.fill", text: "Full search engine with auto-expand and scroll to match")
                    bulletPoint(icon: "checkmark.circle.fill", text: "Split View & Tabbed View modes")
                }
                .padding(.top, 4)
            }
            .frame(maxWidth: 380)
            
            Divider()
                .frame(width: 280)
            
            Button("OK") {
                dismiss()
            }
            .buttonStyle(.borderedProminent)
            .keyboardShortcut(.defaultAction)
        }
        .padding(24)
        .frame(width: 440)
    }
    
    private func bulletPoint(icon: String, text: String) -> some View {
        HStack(alignment: .top, spacing: 6) {
            Image(systemName: icon)
                .foregroundColor(.green)
                .font(.system(size: 11))
            Text(text)
                .font(.system(size: 11))
                .foregroundColor(.primary)
        }
    }
}
