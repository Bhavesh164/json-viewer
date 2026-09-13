import SwiftUI
import AppKit
import JSONViewerCore

@main
struct JSONViewerApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    
    var body: some Scene {
        WindowGroup {
            MainView()
                .navigationTitle("JSON Viewer")
        }
        .windowStyle(.titleBar)
        .windowToolbarStyle(.unified)
        .commands {
            CommandGroup(replacing: .appInfo) {
                Button("About JSON Viewer") {
                    NotificationCenter.default.post(name: .aboutRequested, object: nil)
                }
            }
            
            CommandGroup(replacing: .appSettings) {
                Button("Settings...") {
                    NotificationCenter.default.post(name: .settingsRequested, object: nil)
                }
                .keyboardShortcut(",", modifiers: [.command])
            }
            
            CommandMenu("JSON") {
                Button("Clear Editor") {
                    NotificationCenter.default.post(name: .clearRequested, object: nil)
                }
                .keyboardShortcut("k", modifiers: [.command])
                
                Divider()
                
                Button("Expand All Nodes") {
                    NotificationCenter.default.post(name: .expandAllRequested, object: nil)
                }
                .keyboardShortcut("e", modifiers: [.command])
                
                Button("Collapse All Nodes") {
                    NotificationCenter.default.post(name: .collapseAllRequested, object: nil)
                }
                .keyboardShortcut("e", modifiers: [.command, .shift])
            }
            
            CommandMenu("View") {
                Button("Viewer Tab") {
                    NotificationCenter.default.post(name: .selectTabViewerRequested, object: nil)
                }
                .keyboardShortcut("1", modifiers: [.command])
                
                Button("Text Tab") {
                    NotificationCenter.default.post(name: .selectTabTextRequested, object: nil)
                }
                .keyboardShortcut("2", modifiers: [.command])
                
                Button("Split Tab") {
                    NotificationCenter.default.post(name: .selectTabSplitRequested, object: nil)
                }
                .keyboardShortcut("3", modifiers: [.command])
                
                Divider()
                
                Button("Toggle Properties Panel") {
                    NotificationCenter.default.post(name: .togglePropertiesRequested, object: nil)
                }
                .keyboardShortcut("p", modifiers: [.command, .option])
                
                Divider()
                
                Button("Zoom In") {
                    NotificationCenter.default.post(name: .zoomInRequested, object: nil)
                }
                .keyboardShortcut("+", modifiers: [.command])
                
                Button("Zoom Out") {
                    NotificationCenter.default.post(name: .zoomOutRequested, object: nil)
                }
                .keyboardShortcut("-", modifiers: [.command])
                
                Divider()
                
                Button("Actual Size") {
                    NotificationCenter.default.post(name: .resetZoomRequested, object: nil)
                }
                .keyboardShortcut("0", modifiers: [.command])
            }
            
            CommandGroup(replacing: .help) {
                Button("Keyboard Shortcuts") {
                    NotificationCenter.default.post(name: .shortcutsRequested, object: nil)
                }
                .keyboardShortcut("?", modifiers: [.command])
                
                Divider()
                
                Button("About JSON Viewer") {
                    NotificationCenter.default.post(name: .aboutRequested, object: nil)
                }
            }
        }
        
        Settings {
            SettingsView(settings: .shared, onShowShortcuts: {
                NotificationCenter.default.post(name: .shortcutsRequested, object: nil)
            })
        }
    }
}

class AppDelegate: NSObject, NSApplicationDelegate {
    private var keyMonitor: Any?
    
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        
        // Monitor key down for '?' shortcut
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            if event.characters == "?" {
                // If Command modifier is pressed (Cmd+?), always trigger shortcuts
                if event.modifierFlags.contains(.command) {
                    NotificationCenter.default.post(name: .shortcutsRequested, object: nil)
                    return nil
                }
                
                // If focus is in an editable text field/editor, let '?' type normally!
                if let responder = NSApp.keyWindow?.firstResponder {
                    if responder is NSTextView || responder is NSTextField {
                        return event
                    }
                }
                
                // Otherwise trigger keyboard shortcuts cheatsheet!
                NotificationCenter.default.post(name: .shortcutsRequested, object: nil)
                return nil
            }
            return event
        }
    }
    
    deinit {
        if let monitor = keyMonitor {
            NSEvent.removeMonitor(monitor)
        }
    }
    
    @objc func showAboutDialog() {
        NotificationCenter.default.post(name: .aboutRequested, object: nil)
    }
}

// Notification names for global shortcuts / menu bar commands
extension Notification.Name {
    static let togglePropertiesRequested = Notification.Name("togglePropertiesRequested")
    static let clearRequested = Notification.Name("clearRequested")
    static let expandAllRequested = Notification.Name("expandAllRequested")
    static let collapseAllRequested = Notification.Name("collapseAllRequested")
    static let aboutRequested = Notification.Name("aboutRequested")
    static let zoomInRequested = Notification.Name("zoomInRequested")
    static let zoomOutRequested = Notification.Name("zoomOutRequested")
    static let resetZoomRequested = Notification.Name("resetZoomRequested")
    static let shortcutsRequested = Notification.Name("shortcutsRequested")
    static let settingsRequested = Notification.Name("settingsRequested")
    static let selectTabViewerRequested = Notification.Name("selectTabViewerRequested")
    static let selectTabTextRequested = Notification.Name("selectTabTextRequested")
    static let selectTabSplitRequested = Notification.Name("selectTabSplitRequested")
}
