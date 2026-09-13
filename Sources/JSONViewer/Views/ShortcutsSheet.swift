import SwiftUI
import JSONViewerCore

public struct ShortcutsSheet: View {
    @Environment(\.dismiss) private var dismiss
    @UIState private var searchText: String = ""
    
    public init() {}
    
    struct ShortcutItem: Identifiable {
        let id = UUID()
        let category: String
        let title: String
        let keys: [String]
    }
    
    private let shortcuts: [ShortcutItem] = [
        // General
        ShortcutItem(category: "General", title: "Show Keyboard Shortcuts", keys: ["?"]),
        ShortcutItem(category: "General", title: "Open Settings / Preferences", keys: ["⌘", ","]),
        ShortcutItem(category: "General", title: "Switch to Viewer Tab", keys: ["⌘", "1"]),
        ShortcutItem(category: "General", title: "Switch to Text Editor Tab", keys: ["⌘", "2"]),
        ShortcutItem(category: "General", title: "Switch to Split View Tab", keys: ["⌘", "3"]),
        
        // Tree Viewer
        ShortcutItem(category: "Tree Viewer", title: "Expand All Nodes", keys: ["⌘", "E"]),
        ShortcutItem(category: "Tree Viewer", title: "Collapse All Nodes", keys: ["⌘", "⇧", "E"]),
        ShortcutItem(category: "Tree Viewer", title: "Toggle Properties Panel", keys: ["⌘", "⌥", "P"]),
        ShortcutItem(category: "Tree Viewer", title: "Select Node & Reveal Properties", keys: ["Click"]),
        ShortcutItem(category: "Tree Viewer", title: "Context Menu (Copy Path, Subtree...)", keys: ["Right-Click"]),
        
        // Editor & Formatting
        ShortcutItem(category: "Editor & Formatting", title: "Clear Editor", keys: ["⌘", "K"]),
        ShortcutItem(category: "Editor & Formatting", title: "Open JSON File", keys: ["⌘", "O"]),
        ShortcutItem(category: "Editor & Formatting", title: "Save JSON File", keys: ["⌘", "S"]),
        ShortcutItem(category: "Editor & Formatting", title: "Copy as Python Dictionary", keys: ["⌘", "⌥", "C"]),
        ShortcutItem(category: "Editor & Formatting", title: "Convert JSON to Python", keys: ["JSON → Python"]),
        ShortcutItem(category: "Editor & Formatting", title: "Convert Python to JSON", keys: ["Python → JSON"]),
        ShortcutItem(category: "Editor & Formatting", title: "Zoom In Font Size", keys: ["⌘", "+"]),
        ShortcutItem(category: "Editor & Formatting", title: "Zoom Out Font Size", keys: ["⌘", "-"]),
        ShortcutItem(category: "Editor & Formatting", title: "Reset Zoom / Font Size", keys: ["⌘", "0"]),
        
        // Search
        ShortcutItem(category: "Search & Navigation", title: "Focus Search Bar (quick)", keys: ["/"]),
        ShortcutItem(category: "Search & Navigation", title: "Find in JSON / Focus Search", keys: ["⌘", "F"]),
        ShortcutItem(category: "Search & Navigation", title: "Find Next Match (in search)", keys: ["Enter"]),
        ShortcutItem(category: "Search & Navigation", title: "Find Previous Match (in search)", keys: ["⇧", "Enter"]),
        ShortcutItem(category: "Search & Navigation", title: "Find Next Match (global)", keys: ["⌘", "G"]),
        ShortcutItem(category: "Search & Navigation", title: "Find Previous Match (global)", keys: ["⌘", "⇧", "G"]),
        ShortcutItem(category: "Search & Navigation", title: "Dismiss / Close Modal / Search", keys: ["Esc"])
    ]
    
    private var filteredCategories: [String] {
        let allCategories = ["General", "Tree Viewer", "Editor & Formatting", "Search & Navigation"]
        if searchText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return allCategories
        }
        let query = searchText.lowercased()
        return allCategories.filter { cat in
            shortcuts.contains { $0.category == cat && ($0.title.lowercased().contains(query) || $0.keys.joined().lowercased().contains(query)) }
        }
    }
    
    public var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack(spacing: 12) {
                ZStack {
                    RoundedRectangle(cornerRadius: 8)
                        .fill(Color.accentColor.opacity(0.12))
                        .frame(width: 36, height: 36)
                    Image(systemName: "command")
                        .font(.system(size: 18, weight: .bold))
                        .foregroundColor(.accentColor)
                }
                
                VStack(alignment: .leading, spacing: 2) {
                    Text("Keyboard Shortcuts")
                        .font(.title3.bold())
                    Text("Press '?' anytime outside text fields or '⌘?' anywhere to open this cheatsheet")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                
                Spacer()
                
                // Search filter field
                HStack(spacing: 6) {
                    Image(systemName: "magnifyingglass")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    TextField("Filter shortcuts...", text: $searchText)
                        .textFieldStyle(.plain)
                        .font(.system(size: 12))
                    if !searchText.isEmpty {
                        Button(action: { searchText = "" }) {
                            Image(systemName: "xmark.circle.fill")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 5)
                .background(Color(nsColor: .controlBackgroundColor))
                .cornerRadius(6)
                .overlay(RoundedRectangle(cornerRadius: 6).stroke(Color.secondary.opacity(0.2), lineWidth: 1))
                .frame(width: 180)
            }
            .padding(.horizontal, 20)
            .padding(.top, 18)
            .padding(.bottom, 14)
            
            Divider()
            
            // Shortcuts List by Category
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    ForEach(filteredCategories, id: \.self) { category in
                        VStack(alignment: .leading, spacing: 8) {
                            HStack(spacing: 6) {
                                Image(systemName: iconForCategory(category))
                                    .font(.system(size: 12, weight: .semibold))
                                    .foregroundColor(.accentColor)
                                Text(category)
                                    .font(.system(size: 13, weight: .bold))
                                    .foregroundColor(.primary)
                            }
                            
                            VStack(spacing: 1) {
                                ForEach(shortcuts.filter { item in
                                    item.category == category &&
                                    (searchText.isEmpty || item.title.localizedCaseInsensitiveContains(searchText) || item.keys.joined().localizedCaseInsensitiveContains(searchText))
                                }) { item in
                                    HStack {
                                        Text(item.title)
                                            .font(.system(size: 12))
                                            .foregroundColor(.primary)
                                        
                                        Spacer()
                                        
                                        HStack(spacing: 4) {
                                            ForEach(item.keys, id: \.self) { key in
                                                KeyBadge(key: key)
                                            }
                                        }
                                    }
                                    .padding(.horizontal, 10)
                                    .padding(.vertical, 6)
                                    .background(Color(nsColor: .controlBackgroundColor).opacity(0.5))
                                    .cornerRadius(5)
                                }
                            }
                        }
                    }
                }
                .padding(20)
            }
            .frame(height: 380)
            
            Divider()
            
            // Footer
            HStack {
                Text("Tip: Press 'Esc' to dismiss")
                    .font(.caption)
                    .foregroundColor(.secondary)
                
                Spacer()
                
                Button("Done") {
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.defaultAction)
                .controlSize(.regular)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 12)
            .background(Color(nsColor: .windowBackgroundColor))
        }
        .frame(width: 580)
    }
    
    private func iconForCategory(_ category: String) -> String {
        switch category {
        case "General": return "gearshape.2"
        case "Tree Viewer": return "list.bullet.indent"
        case "Editor & Formatting": return "character.cursor.ibeam"
        case "Search & Navigation": return "magnifyingglass"
        default: return "keyboard"
        }
    }
}

struct KeyBadge: View {
    let key: String
    
    var body: some View {
        Text(key)
            .font(.system(size: 11, weight: .medium, design: .monospaced))
            .foregroundColor(.primary)
            .padding(.horizontal, 6)
            .padding(.vertical, 2.5)
            .background(
                RoundedRectangle(cornerRadius: 4)
                    .fill(Color(nsColor: .textBackgroundColor))
                    .shadow(color: Color.black.opacity(0.08), radius: 1, x: 0, y: 1)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 4)
                    .stroke(Color.secondary.opacity(0.3), lineWidth: 1)
            )
    }
}
