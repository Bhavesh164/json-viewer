import SwiftUI
import AppKit

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
                    NSApp.sendAction(#selector(AppDelegate.showAboutDialog), to: nil, from: nil)
                }
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
        }
    }
}

class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
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
}
