import SwiftUI
import JSONViewerCore

@MainActor
public struct SettingsView: View {
    @ObservedObject var settings: AppSettings
    @Environment(\.dismiss) private var dismiss
    var onShowShortcuts: (() -> Void)?
    
    public init(settings: AppSettings = .shared, onShowShortcuts: (() -> Void)? = nil) {
        self.settings = settings
        self.onShowShortcuts = onShowShortcuts
    }
    
    public var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack(spacing: 12) {
                ZStack {
                    RoundedRectangle(cornerRadius: 8)
                        .fill(Color.accentColor.opacity(0.12))
                        .frame(width: 36, height: 36)
                    Image(systemName: "gearshape.fill")
                        .font(.system(size: 18))
                        .foregroundColor(.accentColor)
                }
                
                VStack(alignment: .leading, spacing: 2) {
                    Text("Settings")
                        .font(.title3.bold())
                    Text("Customize JSON formatting, editor, and viewer behavior")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                
                Spacer()
            }
            .padding(.horizontal, 20)
            .padding(.top, 18)
            .padding(.bottom, 14)
            
            Divider()
            
            // Settings Form
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    
                    // Formatting Section
                    GroupBox {
                        VStack(alignment: .leading, spacing: 12) {
                            HStack {
                                Text("Indentation")
                                    .font(.system(size: 12, weight: .medium))
                                Spacer()
                                Picker("", selection: $settings.indentSpaces) {
                                    Text("2 Spaces (Default)").tag(2)
                                    Text("4 Spaces").tag(4)
                                    Text("Tabs").tag(-1)
                                }
                                .pickerStyle(.menu)
                                .frame(width: 170)
                            }
                            
                            Divider()
                            
                            Toggle(isOn: $settings.escapeSlashesInStringify) {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text("Escape Forward Slashes (\\/)")
                                        .font(.system(size: 12, weight: .medium))
                                    Text("Outputs slashes as '\\/' when stringifying (RFC 8259 compatible)")
                                        .font(.caption)
                                        .foregroundColor(.secondary)
                                }
                            }
                            .toggleStyle(.checkbox)
                            
                            Divider()
                            
                            Toggle(isOn: $settings.sortKeysAlphabetically) {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text("Sort Object Keys Alphabetically")
                                        .font(.system(size: 12, weight: .medium))
                                    Text("Orders keys A-Z during formatting instead of original order")
                                        .font(.caption)
                                        .foregroundColor(.secondary)
                                }
                            }
                            .toggleStyle(.checkbox)
                        }
                        .padding(10)
                    } label: {
                        Label("JSON Formatting & Serialization", systemImage: "curlybraces")
                            .font(.system(size: 12, weight: .bold))
                            .foregroundColor(.accentColor)
                    }
                    
                    // Middle Tab & Parsing Section
                    GroupBox {
                        VStack(alignment: .leading, spacing: 12) {
                            Toggle(isOn: $settings.autoUnwrapStringified) {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text("Auto-Unwrap Stringified JSON")
                                        .font(.system(size: 12, weight: .medium))
                                    Text("Automatically unescapes and renders stringified JSON payloads into the tree")
                                        .font(.caption)
                                        .foregroundColor(.secondary)
                                }
                            }
                            .toggleStyle(.checkbox)
                            
                            Divider()
                            
                            HStack {
                                Text("Default Tab on Launch")
                                    .font(.system(size: 12, weight: .medium))
                                Spacer()
                                Picker("", selection: $settings.defaultTab) {
                                    Text("Text Editor (Middle Tab)").tag("Text")
                                    Text("Tree Viewer").tag("Viewer")
                                    Text("Split View").tag("Split")
                                }
                                .pickerStyle(.menu)
                                .frame(width: 190)
                            }
                        }
                        .padding(10)
                    } label: {
                        Label("General & Parser", systemImage: "slider.horizontal.3")
                            .font(.system(size: 12, weight: .bold))
                            .foregroundColor(.accentColor)
                    }
                    
                    // Editor Section
                    GroupBox {
                        VStack(alignment: .leading, spacing: 12) {
                            HStack {
                                Text("Font Size: \(Int(settings.fontSize)) pt")
                                    .font(.system(size: 12, weight: .medium))
                                    .frame(width: 110, alignment: .leading)
                                
                                Slider(value: $settings.fontSize, in: 9...24, step: 1)
                                
                                Stepper("", value: $settings.fontSize, in: 9...24, step: 1)
                                    .labelsHidden()
                            }
                            
                            Divider()
                            
                            Toggle(isOn: $settings.wrapLines) {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text("Wrap Long Lines")
                                        .font(.system(size: 12, weight: .medium))
                                    Text("Wrap text horizontally instead of horizontal scrolling in code editor")
                                        .font(.caption)
                                        .foregroundColor(.secondary)
                                }
                            }
                            .toggleStyle(.checkbox)
                        }
                        .padding(10)
                    } label: {
                        Label("Editor & Display", systemImage: "textformat")
                            .font(.system(size: 12, weight: .bold))
                            .foregroundColor(.accentColor)
                    }
                    
                    // Shortcuts Link
                    HStack {
                        Button(action: {
                            dismiss()
                            onShowShortcuts?()
                        }) {
                            Label("View Keyboard Shortcuts (?)", systemImage: "keyboard")
                        }
                        .buttonStyle(.link)
                        
                        Spacer()
                        
                        Button("Reset to Defaults") {
                            settings.resetToDefaults()
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                    }
                    .padding(.horizontal, 4)
                }
                .padding(20)
            }
            .frame(height: 390)
            
            Divider()
            
            // Footer
            HStack {
                Spacer()
                Button("Done") {
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.defaultAction)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 12)
            .background(Color(nsColor: .windowBackgroundColor))
        }
        .frame(width: 520)
    }
}
