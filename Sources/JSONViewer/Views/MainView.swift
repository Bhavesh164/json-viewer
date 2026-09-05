import SwiftUI
import JSONViewerCore

public struct MainView: View {
    @StateObject private var model = JSONDocumentModel()
    
    public init() {}
    
    public var body: some View {
        VStack(spacing: 0) {
            // Main Content depending on active tab
            switch model.activeTab {
            case .viewer:
                viewerContentView
            case .text:
                TextEditorView(model: model)
            case .split:
                splitContentView
            }
        }
        .frame(minWidth: 800, minHeight: 540)
        .toolbar {
            ToolbarItemGroup(placement: .principal) {
                Picker("Tab", selection: Binding(
                    get: { model.activeTab },
                    set: { newTab in model.selectTab(newTab) }
                )) {
                    Text("Viewer").tag(AppTab.viewer)
                    Text("Text").tag(AppTab.text)
                    Text("Split").tag(AppTab.split)
                }
                .pickerStyle(.segmented)
                .frame(width: 240)
            }
            
            ToolbarItemGroup(placement: .automatic) {
                Button(action: {
                    model.formatJSON()
                }) {
                    Label("Format", systemImage: "text.alignleft")
                }
                .help("Format / Pretty Print (Cmd+Shift+F)")
                
                Button(action: {
                    model.removeWhitespace()
                }) {
                    Label("Minify", systemImage: "arrow.right.to.line.compact")
                }
                .help("Remove Whitespace (Cmd+Shift+M)")
                
                Button(action: {
                    model.isLoadURLSheetPresented = true
                }) {
                    Label("Load URL", systemImage: "globe")
                }
                .help("Load JSON from URL (Cmd+L)")
                
                Button(action: {
                    model.isSearchVisible.toggle()
                }) {
                    Label("Find", systemImage: "magnifyingglass")
                }
                .help("Toggle Search Bar (Cmd+F)")
            }
        }
        .alert("JSON error", isPresented: $model.isErrorAlertPresented) {
            Button("OK", role: .cancel) { }
        } message: {
            Text(model.errorMessage)
        }
        .sheet(isPresented: $model.isLoadURLSheetPresented) {
            LoadURLSheet(model: model)
        }
        .sheet(isPresented: $model.isAboutSheetPresented) {
            AboutSheet()
        }
        .onReceive(NotificationCenter.default.publisher(for: .formatRequested)) { _ in
            model.formatJSON()
        }
        .onReceive(NotificationCenter.default.publisher(for: .minifyRequested)) { _ in
            model.removeWhitespace()
        }
        .onReceive(NotificationCenter.default.publisher(for: .clearRequested)) { _ in
            model.clearText()
        }
        .onReceive(NotificationCenter.default.publisher(for: .loadURLRequested)) { _ in
            model.isLoadURLSheetPresented = true
        }
        .onReceive(NotificationCenter.default.publisher(for: .expandAllRequested)) { _ in
            model.expandAll()
        }
        .onReceive(NotificationCenter.default.publisher(for: .collapseAllRequested)) { _ in
            model.collapseAll()
        }
        .onReceive(NotificationCenter.default.publisher(for: .aboutRequested)) { _ in
            model.isAboutSheetPresented = true
        }
        .onReceive(NotificationCenter.default.publisher(for: .zoomInRequested)) { _ in
            model.zoomIn()
        }
        .onReceive(NotificationCenter.default.publisher(for: .zoomOutRequested)) { _ in
            model.zoomOut()
        }
        .onReceive(NotificationCenter.default.publisher(for: .resetZoomRequested)) { _ in
            model.resetZoom()
        }
    }
    
    // MARK: - Viewer Mode (Tree + Property Grid + Bottom Search)
    private var viewerContentView: some View {
        VStack(spacing: 0) {
            HSplitView {
                TreeViewer(model: model)
                    .frame(minWidth: 320, maxWidth: .infinity)
                
                PropertyGridView(model: model)
                    .frame(minWidth: 260, idealWidth: 340, maxWidth: 600)
            }
            
            if model.isSearchVisible {
                SearchToolbar(model: model)
            }
        }
    }
    
    // MARK: - Modern Split Mode (Live Editor on Left + Tree & Grid on Right)
    private var splitContentView: some View {
        HSplitView {
            TextEditorView(model: model)
                .frame(minWidth: 360, maxWidth: .infinity)
            
            VStack(spacing: 0) {
                HSplitView {
                    TreeViewer(model: model)
                        .frame(minWidth: 280, maxWidth: .infinity)
                    
                    PropertyGridView(model: model)
                        .frame(minWidth: 220, idealWidth: 280, maxWidth: 500)
                }
                
                if model.isSearchVisible {
                    SearchToolbar(model: model)
                }
            }
            .frame(minWidth: 500, maxWidth: .infinity)
        }
    }
}
