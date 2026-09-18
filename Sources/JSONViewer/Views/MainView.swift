import SwiftUI
import JSONViewerCore

public struct MainView: View {
    @StateObject private var model = JSONDocumentModel()
    
    public init() {}
    
    public var body: some View {
        mainContent
            .frame(minWidth: 800, minHeight: 540)
            .toolbar { toolbarContent }
            .alert("JSON error", isPresented: $model.isErrorAlertPresented) {
                Button("OK", role: .cancel) { }
            } message: {
                Text(model.errorMessage)
            }
            .modifier(MainSheetsModifier(model: model))
            .modifier(ViewControlsNotificationModifier(model: model))
            .modifier(SearchAndFileNotificationModifier(model: model))
    }
    
    @ViewBuilder
    private var mainContent: some View {
        VStack(spacing: 0) {
            switch model.activeTab {
            case .viewer:
                viewerContentView
            case .text:
                TextEditorView(model: model)
            case .split:
                splitContentView
            }
        }
    }
    
    @ToolbarContentBuilder
    private var toolbarContent: some ToolbarContent {
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
                model.toggleProperties()
            }) {
                Label(model.isPropertiesVisible ? "Hide Properties" : "Show Properties", systemImage: "sidebar.trailing")
            }
            .help(model.isPropertiesVisible ? "Hide Properties Panel (Cmd+Option+P)" : "Show Properties Panel (Cmd+Option+P)")
            
            Button(action: {
                if model.isSearchVisible {
                    model.isSearchVisible = false
                } else {
                    model.focusSearch()
                }
            }) {
                Label("Find", systemImage: "magnifyingglass")
            }
            .help("Toggle & Focus Search Bar (/ or Cmd+F)")
            
            Button(action: {
                model.isShortcutsSheetPresented = true
            }) {
                Label("Shortcuts", systemImage: "questionmark.circle")
            }
            .help("Keyboard Shortcuts (?)")
            
            Button(action: {
                model.isSettingsSheetPresented = true
            }) {
                Label("Settings", systemImage: "gearshape")
            }
            .help("Settings (Cmd+,)")
        }
    }
    
    // MARK: - Viewer Mode (Tree + Property Grid + Bottom Search)
    private var viewerContentView: some View {
        VStack(spacing: 0) {
            HSplitView {
                TreeViewer(model: model)
                    .frame(minWidth: 320, maxWidth: .infinity)
                
                if model.isPropertiesVisible {
                    PropertyGridView(model: model)
                        .frame(minWidth: 260, idealWidth: 340, maxWidth: 600)
                }
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
                    
                    if model.isPropertiesVisible {
                        PropertyGridView(model: model)
                            .frame(minWidth: 220, idealWidth: 280, maxWidth: 500)
                    }
                }
                
                if model.isSearchVisible {
                    SearchToolbar(model: model)
                }
            }
            .frame(minWidth: 500, maxWidth: .infinity)
        }
    }
}

// MARK: - Sheets Modifier
struct MainSheetsModifier: ViewModifier {
    @ObservedObject var model: JSONDocumentModel
    
    func body(content: Content) -> some View {
        content
            .sheet(isPresented: $model.isAboutSheetPresented) {
                AboutSheet()
            }
            .sheet(isPresented: $model.isShortcutsSheetPresented) {
                ShortcutsSheet()
            }
            .sheet(isPresented: $model.isSettingsSheetPresented) {
                SettingsView(settings: model.settings, onShowShortcuts: {
                    model.isShortcutsSheetPresented = true
                })
            }
    }
}

// MARK: - View Controls Notifications Modifier
struct ViewControlsNotificationModifier: ViewModifier {
    @ObservedObject var model: JSONDocumentModel
    
    func body(content: Content) -> some View {
        content
            .onReceive(NotificationCenter.default.publisher(for: .togglePropertiesRequested)) { _ in
                model.toggleProperties()
            }
            .onReceive(NotificationCenter.default.publisher(for: .clearRequested)) { _ in
                model.clearText()
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
            .onReceive(NotificationCenter.default.publisher(for: .shortcutsRequested)) { _ in
                model.isShortcutsSheetPresented.toggle()
            }
            .onReceive(NotificationCenter.default.publisher(for: .settingsRequested)) { _ in
                model.isSettingsSheetPresented = true
            }
            .onReceive(NotificationCenter.default.publisher(for: .selectTabViewerRequested)) { _ in
                model.selectTab(.viewer)
            }
            .onReceive(NotificationCenter.default.publisher(for: .selectTabTextRequested)) { _ in
                model.selectTab(.text)
            }
            .onReceive(NotificationCenter.default.publisher(for: .selectTabSplitRequested)) { _ in
                model.selectTab(.split)
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
}

// MARK: - Search & File Notifications Modifier
struct SearchAndFileNotificationModifier: ViewModifier {
    @ObservedObject var model: JSONDocumentModel
    
    func body(content: Content) -> some View {
        content
            .onReceive(NotificationCenter.default.publisher(for: .focusSearchRequested)) { _ in
                if model.activeTab == .text {
                    model.selectTab(.split)
                }
                model.focusSearch()
            }
            .onReceive(NotificationCenter.default.publisher(for: .closeSearchRequested)) { _ in
                model.isSearchVisible = false
                NSApp.keyWindow?.makeFirstResponder(nil)
            }
            .onReceive(NotificationCenter.default.publisher(for: .findNextRequested)) { _ in
                model.searchNext()
            }
            .onReceive(NotificationCenter.default.publisher(for: .findPreviousRequested)) { _ in
                model.searchPrevious()
            }
            .onReceive(NotificationCenter.default.publisher(for: .openFileRequested)) { _ in
                promptOpenFile(model: model)
            }
            .onReceive(NotificationCenter.default.publisher(for: .saveFileRequested)) { _ in
                promptSaveFile(model: model)
            }
            .onReceive(NotificationCenter.default.publisher(for: .copyPythonObjectRequested)) { _ in
                model.copyPythonObject()
            }
    }
}

// MARK: - Global File Open / Save Dialog Helpers
@MainActor
func promptOpenFile(model: JSONDocumentModel) {
    let panel = NSOpenPanel()
    panel.allowedContentTypes = [.json, .plainText]
    panel.allowsMultipleSelection = false
    panel.canChooseDirectories = false
    if panel.runModal() == .OK, let url = panel.url {
        model.openFile(url: url)
    }
}

@MainActor
func promptSaveFile(model: JSONDocumentModel) {
    let panel = NSSavePanel()
    panel.allowedContentTypes = [.json]
    panel.nameFieldStringValue = "document.json"
    if panel.runModal() == .OK, let url = panel.url {
        model.saveToFile(url: url)
    }
}
