import SwiftUI
import AppKit
import JSONViewerCore

public struct TreeViewer: View {
    @ObservedObject var model: JSONDocumentModel
    
    public init(model: JSONDocumentModel) {
        self.model = model
    }
    
    public var body: some View {
        VStack(spacing: 0) {
            // Tree Controls Bar
            HStack(spacing: 8) {
                Button(action: {
                    model.expandAll()
                }) {
                    Label("Expand All", systemImage: "plus.square")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                
                Button(action: {
                    model.collapseAll()
                }) {
                    Label("Collapse All", systemImage: "minus.square")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                
                Divider()
                    .frame(height: 14)
                
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
                
                Spacer()
                
                if let selected = model.selectedNode {
                    Text(selected.path)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundColor(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(Color(nsColor: .controlBackgroundColor))
            
            Divider()
            
            // Tree Scroll Area with Virtualized Flat Rows
            if model.rootNode != nil {
                ScrollViewReader { proxy in
                    GeometryReader { geo in
                        ScrollView([.vertical, .horizontal]) {
                            LazyVStack(alignment: .leading, spacing: 1) {
                                ForEach(model.visibleTreeRows) { row in
                                    FlatTreeNodeRow(
                                        row: row,
                                        isSelected: model.selectedNode?.id == row.node.id,
                                        isMatch: model.searchResultIds.contains(row.node.id),
                                        isLeafExpanded: model.expandedLeafNodeIds.contains(row.node.id),
                                        fontSize: model.fontSize,
                                        viewportWidth: geo.size.width,
                                        model: model
                                    )
                                    .equatable()
                                }
                            }
                            .padding(.vertical, 4)
                            .padding(.leading, 8)
                            .padding(.trailing, 32)
                            .frame(minWidth: geo.size.width, minHeight: geo.size.height, alignment: .topLeading)
                        }
                        .background(Color(nsColor: .textBackgroundColor))
                        .onChange(of: model.selectedNode?.id) { selectedId in
                            if let selectedId = selectedId {
                                withAnimation(.easeInOut(duration: 0.15)) {
                                    proxy.scrollTo(selectedId, anchor: .center)
                                }
                            }
                        }
                    }
                }
            } else {
                VStack(spacing: 12) {
                    Spacer()
                    Image(systemName: "curlybraces.square")
                        .font(.system(size: 36))
                        .foregroundColor(.secondary.opacity(0.5))
                    Text("No JSON tree loaded")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundColor(.secondary)
                    Button("Parse & View JSON") {
                        model.parseAndBuildTree(silent: false)
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    Spacer()
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
    }
}

struct FlatTreeNodeRow: View, Equatable {
    let row: FlatTreeRow
    let isSelected: Bool
    let isMatch: Bool
    let isLeafExpanded: Bool
    let fontSize: CGFloat
    var viewportWidth: CGFloat = 800
    let model: JSONDocumentModel
    
    @State private var isKeyHovered: Bool = false
    @State private var isCopied: Bool = false
    
    static func == (lhs: FlatTreeNodeRow, rhs: FlatTreeNodeRow) -> Bool {
        lhs.row.node.id == rhs.row.node.id &&
        lhs.row.depth == rhs.row.depth &&
        lhs.row.isExpanded == rhs.row.isExpanded &&
        lhs.isSelected == rhs.isSelected &&
        lhs.isMatch == rhs.isMatch &&
        lhs.isLeafExpanded == rhs.isLeafExpanded &&
        lhs.fontSize == rhs.fontSize &&
        lhs.viewportWidth == rhs.viewportWidth
    }
    
    private var isBigText: Bool {
        if case .string(let s) = row.node.value {
            return s.count > 40 || s.contains("\n")
        }
        return false
    }
    
    private var expandedBoxWidth: CGFloat {
        let leadingIndent = 12.0 + CGFloat(row.depth * 18) + 18.0 + max(16.0, fontSize + 4.0) + 4.0
        let trailingMargin = 36.0
        let available = viewportWidth - leadingIndent - trailingMargin
        return max(320.0, available)
    }
    
    var body: some View {
        HStack(alignment: isLeafExpanded ? .top : .center, spacing: 4) {
            // Indentation
            if row.depth > 0 {
                Color.clear
                    .frame(width: CGFloat(row.depth * 18), height: 1)
            }
            
            // Classic disclosure toggle: [-] when expanded, [+] when collapsed
            if row.isContainer {
                Button(action: {
                    model.toggleExpand(nodeId: row.node.id)
                }) {
                    Image(systemName: row.isExpanded ? "minus.square" : "plus.square")
                        .font(.system(size: max(10, fontSize - 1), weight: .medium))
                        .foregroundColor(.secondary)
                        .frame(width: 14, height: 14)
                }
                .buttonStyle(.plain)
            } else if isBigText {
                // Visual toggle icon for big text
                Button(action: {
                    withAnimation(.easeInOut(duration: 0.15)) {
                        model.toggleExpandLeaf(nodeId: row.node.id)
                    }
                }) {
                    Image(systemName: isLeafExpanded ? "text.badge.minus" : "text.badge.plus")
                        .font(.system(size: max(10, fontSize - 1), weight: .medium))
                        .foregroundColor(.accentColor)
                        .frame(width: 14, height: 14)
                }
                .buttonStyle(.plain)
                .help(isLeafExpanded ? "Click to collapse full text" : "Click to expand full text")
            } else {
                Color.clear
                .frame(width: 14, height: 1)
            }
            
            // Type Icon Badge
            ZStack {
                RoundedRectangle(cornerRadius: 3)
                    .fill(row.node.typeColor.opacity(0.18))
                    .frame(width: max(16, fontSize + 4), height: max(16, fontSize + 4))
                
                Image(systemName: row.node.systemIconName)
                    .font(.system(size: max(8, fontSize - 3), weight: .bold))
                    .foregroundColor(row.node.typeColor)
            }
            
            // Content
            if row.isContainer {
                HStack(spacing: 4) {
                    Text(row.node.key == "JSON" ? "JSON" : row.node.key)
                        .font(.system(size: fontSize, weight: .medium, design: .monospaced))
                        .foregroundColor(.primary)
                        .lineLimit(1)
                    
                    Text(row.node.typeBadgeText)
                        .font(.system(size: max(9, fontSize - 2), weight: .semibold, design: .monospaced))
                        .foregroundColor(row.node.typeColor)
                        .padding(.horizontal, 4)
                        .padding(.vertical, 1)
                        .background(row.node.typeColor.opacity(0.12))
                        .cornerRadius(3)
                        .lineLimit(1)
                }
                .fixedSize(horizontal: true, vertical: false)
            } else if isBigText && isLeafExpanded {
                // Expanded Big Text View with multi-line wrap, copy, and collapse
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 6) {
                        Button(action: {
                            withAnimation(.easeInOut(duration: 0.15)) {
                                model.toggleExpandLeaf(nodeId: row.node.id)
                            }
                        }) {
                            HStack(spacing: 2) {
                                Text(row.node.key)
                                    .font(.system(size: fontSize, weight: .bold, design: .monospaced))
                                    .foregroundColor(.accentColor)
                                    .underline()
                                Text(":")
                                    .font(.system(size: fontSize, design: .monospaced))
                                    .foregroundColor(.secondary)
                            }
                        }
                        .buttonStyle(.plain)
                        .help("Click to collapse text")
                        
                        Button(action: {
                            withAnimation(.easeInOut(duration: 0.15)) {
                                model.toggleExpandLeaf(nodeId: row.node.id)
                            }
                        }) {
                            HStack(spacing: 3) {
                                Image(systemName: "chevron.up")
                                    .font(.system(size: 8, weight: .bold))
                                Text("collapse")
                                    .font(.system(size: max(8, fontSize - 3), weight: .semibold))
                            }
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Color.secondary.opacity(0.16))
                            .foregroundColor(.secondary)
                            .cornerRadius(4)
                        }
                        .buttonStyle(.plain)
                        .help("Click to collapse text")
                        
                        Button(action: {
                            copyToClipboard(row.node.valueString)
                            isCopied = true
                            DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) {
                                isCopied = false
                            }
                        }) {
                            HStack(spacing: 3) {
                                Image(systemName: isCopied ? "checkmark" : "doc.on.doc")
                                Text(isCopied ? "Copied!" : "Copy")
                            }
                            .font(.system(size: max(8, fontSize - 3)))
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.mini)
                        
                        if case .string(let str) = row.node.value {
                            Text("\(str.count) chars")
                                .font(.system(size: max(8, fontSize - 3), design: .monospaced))
                                .foregroundColor(.secondary.opacity(0.6))
                        }
                        
                        Spacer()
                    }
                    .frame(width: expandedBoxWidth, alignment: .leading)
                    
                    Text(row.node.valueStringRepresentation)
                        .font(.system(size: fontSize, design: .monospaced))
                        .foregroundColor(row.node.typeColor)
                        .lineLimit(nil)
                        .textSelection(.enabled)
                        .padding(10)
                        .frame(width: expandedBoxWidth, alignment: .leading)
                        .background(Color.secondary.opacity(0.08))
                        .overlay(
                            RoundedRectangle(cornerRadius: 6)
                                .stroke(Color.accentColor.opacity(0.35), lineWidth: 1)
                        )
                        .cornerRadius(6)
                }
                .frame(width: expandedBoxWidth, alignment: .leading)
            } else {
                // Collapsed Leaf row (standard single line with expand indicator if big text)
                HStack(spacing: 4) {
                    if isBigText {
                        Button(action: {
                            withAnimation(.easeInOut(duration: 0.15)) {
                                model.toggleExpandLeaf(nodeId: row.node.id)
                            }
                        }) {
                            HStack(spacing: 2) {
                                Text(row.node.key)
                                    .font(.system(size: fontSize, weight: .semibold, design: .monospaced))
                                    .foregroundColor(isKeyHovered ? .accentColor : .primary)
                                    .underline(isKeyHovered)
                                Text(":")
                                    .font(.system(size: fontSize, design: .monospaced))
                                    .foregroundColor(.secondary)
                            }
                        }
                        .buttonStyle(.plain)
                        .onHover { isKeyHovered = $0 }
                        .help("Click key to expand full text")
                    } else {
                        Text(row.node.key)
                            .font(.system(size: fontSize, weight: .medium, design: .monospaced))
                            .foregroundColor(.primary)
                            .lineLimit(1)
                        Text(":")
                            .font(.system(size: fontSize, design: .monospaced))
                            .foregroundColor(.secondary)
                            .lineLimit(1)
                    }
                    
                    Text(row.node.valueStringRepresentation)
                        .font(.system(size: fontSize, design: .monospaced))
                        .foregroundColor(row.node.typeColor)
                        .lineLimit(1)
                    
                    if isBigText {
                        Button(action: {
                            withAnimation(.easeInOut(duration: 0.15)) {
                                model.toggleExpandLeaf(nodeId: row.node.id)
                            }
                        }) {
                            HStack(spacing: 2) {
                                Image(systemName: "chevron.down")
                                    .font(.system(size: 8, weight: .bold))
                                Text("expand")
                                    .font(.system(size: max(8, fontSize - 3), weight: .semibold))
                            }
                            .padding(.horizontal, 5)
                            .padding(.vertical, 1.5)
                            .background(Color.accentColor.opacity(0.16))
                            .foregroundColor(.accentColor)
                            .cornerRadius(3)
                        }
                        .buttonStyle(.plain)
                        .help("Click to see full text")
                    }
                }
                .fixedSize(horizontal: true, vertical: false)
            }
            
            if isMatch {
                Text("MATCH")
                    .font(.system(size: 8, weight: .black))
                    .foregroundColor(.white)
                    .padding(.horizontal, 4)
                    .padding(.vertical, 1)
                    .background(Color.accentColor)
                    .cornerRadius(3)
                    .lineLimit(1)
            }
        }
        .padding(.vertical, 2)
        .padding(.horizontal, 4)
        .fixedSize(horizontal: !isLeafExpanded, vertical: false)
        .background(
            RoundedRectangle(cornerRadius: 4)
                .fill(isSelected ? Color.accentColor.opacity(0.2) : (isMatch ? Color.yellow.opacity(0.15) : Color.clear))
        )
        .contentShape(Rectangle())
        .id(row.node.id)
        .onTapGesture {
            model.selectedNode = row.node
        }
        .contextMenu {
            if row.isContainer {
                Button(row.isExpanded ? "Collapse" : "Expand") {
                    model.toggleExpand(nodeId: row.node.id)
                }
                Button("Expand All Sub-levels") {
                    model.expandSubtree(row.node)
                }
                Button("Collapse All Sub-levels") {
                    model.collapseSubtree(row.node)
                }
                Divider()
            }
            
            Button("Copy Value") {
                copyToClipboard(row.node.formattedValueString)
            }
            Button("Copy Key") {
                copyToClipboard(row.node.key)
            }
            Button("Copy JSON Path") {
                copyToClipboard(row.node.path)
            }
            Button("Copy Subtree as JSON") {
                copyToClipboard(row.node.value.format(indentSpaces: 2))
            }
        }
    }
    
    private func copyToClipboard(_ str: String) {
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(str, forType: .string)
    }
}

extension JSONNode {
    var valueStringRepresentation: String {
        switch value {
        case .string(let s):
            return "\"\(s)\""
        case .number(_, let raw):
            return raw
        case .bool(let b):
            return b ? "true" : "false"
        case .null:
            return "null"
        case .object, .array:
            return ""
        }
    }
}
