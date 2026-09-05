import SwiftUI
import AppKit
import JSONViewerCore

public struct PropertyGridView: View {
    @ObservedObject var model: JSONDocumentModel
    @State private var filterQuery: String = ""
    @State private var copiedRowId: Int?
    @State private var displayLimit: Int = 500
    
    public init(model: JSONDocumentModel) {
        self.model = model
    }
    
    private var allFilteredRows: [PropertyGridRow] {
        if filterQuery.isEmpty {
            return model.selectedNodeProperties
        }
        let q = filterQuery.lowercased()
        return model.selectedNodeProperties.filter { $0.name.lowercased().contains(q) || $0.value.lowercased().contains(q) }
    }
    
    private var displayedRows: [PropertyGridRow] {
        let all = allFilteredRows
        if all.count <= displayLimit {
            return all
        }
        return Array(all.prefix(displayLimit))
    }
    
    public var body: some View {
        VStack(spacing: 0) {
            // Header Bar
            HStack(spacing: 6) {
                Label {
                    Text("Properties")
                        .font(.system(size: 12, weight: .bold))
                        .lineLimit(1)
                } icon: {
                    Image(systemName: "list.dash.header.rectangle")
                        .foregroundColor(.accentColor)
                }
                .fixedSize(horizontal: true, vertical: false)
                
                if let selected = model.selectedNode {
                    Text(selected.key == "JSON" ? "JSON (Root)" : selected.key)
                        .font(.system(size: 11, design: .monospaced))
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Color.secondary.opacity(0.12))
                        .cornerRadius(4)
                }
                
                if let parent = model.selectedNode?.parent {
                    Button(action: {
                        model.navigateToParent()
                    }) {
                        HStack(spacing: 2) {
                            Image(systemName: "chevron.left")
                            Text(parent.key == "JSON" ? "JSON (Root)" : parent.key)
                                .font(.system(size: 10, design: .monospaced))
                                .lineLimit(1)
                        }
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.mini)
                    .help("Jump back to \(parent.key == "JSON" ? "JSON (Root)" : parent.key)")
                }
                
                Spacer(minLength: 4)
                
                Text("\(allFilteredRows.count) items")
                    .font(.system(size: 11))
                    .foregroundColor(.secondary)
                    .lineLimit(1)
                    .fixedSize(horizontal: true, vertical: false)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(Color(nsColor: .controlBackgroundColor))
            
            Divider()
            
            // Search filter for properties
            HStack {
                Image(systemName: "magnifyingglass")
                    .foregroundColor(.secondary)
                    .font(.system(size: 10))
                TextField("Filter properties...", text: $filterQuery)
                    .textFieldStyle(.plain)
                    .font(.system(size: 11))
                if !filterQuery.isEmpty {
                    Button(action: { filterQuery = "" }) {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundColor(.secondary)
                            .font(.system(size: 10))
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(Color(nsColor: .textBackgroundColor).opacity(0.6))
            
            Divider()
            
            // Column Headers
            HStack {
                Text("Name")
                    .font(.system(size: max(10, model.fontSize - 1), weight: .semibold))
                    .foregroundColor(.secondary)
                    .frame(width: max(110, model.fontSize * 9.5), alignment: .leading)
                
                Divider()
                    .frame(height: 12)
                
                Text("Value")
                    .font(.system(size: max(10, model.fontSize - 1), weight: .semibold))
                    .foregroundColor(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 4)
            .background(Color(nsColor: .controlBackgroundColor).opacity(0.5))
            
            Divider()
            
            // Rows
            if allFilteredRows.isEmpty {
                VStack(spacing: 8) {
                    Spacer()
                    Image(systemName: "tray")
                        .font(.system(size: 24))
                        .foregroundColor(.secondary.opacity(0.5))
                    Text(model.selectedNode == nil ? "Select an item to view its properties" : "No direct properties")
                        .font(.system(size: 12))
                        .foregroundColor(.secondary)
                    Spacer()
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(displayedRows) { row in
                            PropertyRowView(
                                row: row,
                                fontSize: max(10, model.fontSize - 1),
                                isCopied: copiedRowId == row.id,
                                onSelect: {
                                    model.navigateToProperty(row)
                                }
                            )
                            .contextMenu {
                                Button("Go to Element in Tree") {
                                    model.navigateToProperty(row)
                                }
                                Divider()
                                Button("Copy Value") {
                                    copyToClipboard(row.value)
                                    flashCopied(row.id)
                                }
                                Button("Copy Name") {
                                    copyToClipboard(row.name)
                                    flashCopied(row.id)
                                }
                                Button("Copy JSON Path") {
                                    copyToClipboard(row.path)
                                    flashCopied(row.id)
                                }
                            }
                            Divider()
                        }
                        
                        if allFilteredRows.count > displayLimit {
                            Button("Show more items (\(displayLimit) of \(allFilteredRows.count) shown)...") {
                                displayLimit += 500
                            }
                            .buttonStyle(.bordered)
                            .controlSize(.small)
                            .padding(.vertical, 8)
                        }
                    }
                }
            }
        }
        .background(Color(nsColor: .textBackgroundColor))
    }
    
    private func copyToClipboard(_ text: String) {
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(text, forType: .string)
    }
    
    private func flashCopied(_ id: Int) {
        copiedRowId = id
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) {
            if copiedRowId == id {
                copiedRowId = nil
            }
        }
    }
}

struct PropertyRowView: View {
    let row: PropertyGridRow
    let fontSize: CGFloat
    let isCopied: Bool
    var onSelect: (() -> Void)? = nil
    @State private var isNameHovered: Bool = false
    @State private var isRowHovered: Bool = false
    @State private var isExpandedValue: Bool = false
    
    private var isBigValue: Bool {
        row.value.count > 35 || row.value.contains("\n")
    }
    
    var body: some View {
        HStack(alignment: isExpandedValue ? .top : .center, spacing: 0) {
            // Name (Clickable link to element in tree)
            Button(action: {
                onSelect?()
            }) {
                HStack(spacing: 4) {
                    Text(row.name)
                        .font(.system(size: fontSize, weight: .semibold, design: .monospaced))
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .foregroundColor(isNameHovered ? .accentColor : .primary)
                        .underline(isNameHovered)
                    
                    Spacer(minLength: 0)
                    
                    if isNameHovered {
                        Image(systemName: "arrow.right.circle.fill")
                            .font(.system(size: max(9, fontSize - 2)))
                            .foregroundColor(.accentColor)
                    }
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .onHover { hovering in
                isNameHovered = hovering
            }
            .help("Click to jump to '\(row.name)' in Tree")
            .frame(width: max(110, fontSize * 9.5), alignment: .leading)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            
            Rectangle()
                .fill(Color(nsColor: .separatorColor))
                .frame(width: 1)
            
            // Value
            VStack(alignment: .leading, spacing: 2) {
                HStack(alignment: isExpandedValue ? .top : .center, spacing: 4) {
                    if isExpandedValue {
                        Text(row.value)
                            .font(.system(size: fontSize, design: .monospaced))
                            .foregroundColor(colorForType(row.type))
                            .lineLimit(nil)
                            .textSelection(.enabled)
                    } else {
                        Text(row.value)
                            .font(.system(size: fontSize, design: .monospaced))
                            .foregroundColor(colorForType(row.type))
                            .lineLimit(1)
                            .truncationMode(.tail)
                    }
                    
                    Spacer(minLength: 0)
                    
                    if isBigValue {
                        Button(action: {
                            withAnimation(.easeInOut(duration: 0.15)) {
                                isExpandedValue.toggle()
                            }
                        }) {
                            HStack(spacing: 2) {
                                Image(systemName: isExpandedValue ? "chevron.up" : "chevron.down")
                                    .font(.system(size: 8, weight: .bold))
                                Text(isExpandedValue ? "less" : "more")
                                    .font(.system(size: max(8, fontSize - 3), weight: .semibold))
                            }
                            .padding(.horizontal, 4)
                            .padding(.vertical, 1)
                            .background(Color.accentColor.opacity(0.14))
                            .foregroundColor(.accentColor)
                            .cornerRadius(3)
                        }
                        .buttonStyle(.plain)
                        .help(isExpandedValue ? "Click to show less" : "Click to expand full text")
                    }
                    
                    if isCopied {
                        Text("Copied!")
                            .font(.system(size: max(8, fontSize - 2), weight: .bold))
                            .foregroundColor(.accentColor)
                            .padding(.horizontal, 4)
                            .padding(.vertical, 1)
                            .background(Color.accentColor.opacity(0.15))
                            .cornerRadius(3)
                    }
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(isRowHovered ? Color.secondary.opacity(0.08) : Color.clear)
        .contentShape(Rectangle())
        .onHover { hovering in
            isRowHovered = hovering
        }
        .onTapGesture(count: 2) {
            onSelect?()
        }
    }
    
    private func colorForType(_ type: String) -> Color {
        switch type {
        case "string": return Color(red: 0.05, green: 0.45, blue: 0.95)
        case "number": return Color(red: 0.15, green: 0.65, blue: 0.35)
        case "boolean": return Color(red: 0.85, green: 0.50, blue: 0.05)
        case "null": return Color(red: 0.85, green: 0.20, blue: 0.20)
        default: return .secondary
        }
    }
}
