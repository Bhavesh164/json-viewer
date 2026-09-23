//! Linux egui UI for JSON Viewer — complete functional, architectural, and visual
//! parity with the macOS SwiftUI app (`Sources/JSONViewer/Views`).
//!
//! Features:
//! - Centered segmented tab navigation (`Viewer` | `Text` | `Split`) with header tools
//! - Virtualized Tree Viewer with depth indentation, disclosure toggles, colored type badges,
//!   inline expandable big-text panels, search match tags, and rich context menus
//! - Collapsible Property Grid with "Jump back to parent", element jump links, and search filter
//! - Bottom-docked Search Bar with live match counts, Next/Previous, and Enter/Shift+Enter shortcuts
//! - Dedicated Text Tab with Paste/Copy dropdown/Clear, Format/Minify/Stringify/Unescape, JSON↔Python
//! - Modern Split View with live two-way synchronization
//! - Global shortcuts parity matching macOS (Ctrl+1/2/3, Ctrl+F, Ctrl+G, Ctrl+E, Ctrl+Alt+P, Ctrl+O, Ctrl+S)

use crate::clipboard;
use crate::model::{AppTab, DocumentModel, FlatTreeRow, PropertyRow};
use crate::settings::Settings;
use std::time::Instant;

pub struct ViewerApp {
    pub doc: DocumentModel,
    pub settings: Settings,
    pub editor_text: String,
    pub split_text: String,
    pub search_input: String,
    pub show_props: bool,
    pub show_search: bool,
    pub show_settings: bool,
    pub show_shortcuts: bool,
    pub show_about: bool,
    pub prop_filter: String,
    pub shortcut_filter: String,
    pub toast: Option<(String, Instant)>,
    pub last_edit: Instant,
    pub pending_reparse: bool,
    pub focus_search: bool,
    pub initialized: bool,
    pub last_tab: AppTab,
}

impl ViewerApp {
    pub fn new(settings: Settings, initial_file: Option<String>) -> Self {
        let mut doc = DocumentModel::new(&settings);
        if let Some(path) = initial_file {
            let p = std::path::PathBuf::from(&path);
            if p.is_file() {
                let _ = doc.load_file(&p, &settings);
            } else if !path.trim().is_empty() {
                doc.raw_text = path;
                doc.update_metrics();
                let _ = doc.parse_and_build_tree(true, &settings);
            }
        }
        let editor_text = doc.raw_text.clone();
        let split_text = doc.raw_text.clone();
        let initial_tab = doc.active_tab;

        Self {
            doc,
            settings: settings.clone(),
            editor_text,
            split_text,
            search_input: String::new(),
            show_props: true,
            show_search: true,
            show_settings: false,
            show_shortcuts: false,
            show_about: false,
            prop_filter: String::new(),
            shortcut_filter: String::new(),
            toast: None,
            last_edit: Instant::now(),
            pending_reparse: false,
            focus_search: false,
            initialized: false,
            last_tab: initial_tab,
        }
    }

    fn save_settings(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_vec_pretty(&self.settings) {
            let _ = std::fs::write(&path, data);
        }
    }

    fn sync_editors_from_doc(&mut self) {
        self.editor_text = self.doc.raw_text.clone();
        self.split_text = self.doc.raw_text.clone();
        self.pending_reparse = false;
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    fn copy_out(&mut self, ctx: &egui::Context, text: String, label: &str) {
        ctx.copy_text(text.clone());
        let ok = clipboard::set_text(&text);
        if ok || !text.is_empty() {
            self.toast(format!("{} — copied!", label));
        } else {
            self.toast(format!("{} — copy failed", label));
        }
    }

    pub fn do_search_go(&mut self) {
        self.doc.search_query = self.search_input.clone();
        self.doc.search_start(&self.settings);
    }

    pub fn do_search_next(&mut self) {
        self.doc.search_query = self.search_input.clone();
        self.doc.search_next(&self.settings);
    }

    pub fn do_search_prev(&mut self) {
        self.doc.search_query = self.search_input.clone();
        self.doc.search_previous(&self.settings);
    }

    pub fn apply_transform(&mut self, kind: &str) {
        let res = match kind {
            "format" => self.doc.beautify(&self.settings),
            "minify" => self.doc.minify(&self.settings),
            "j2p" => self.doc.json_to_python(&self.settings),
            "p2j" => self.doc.python_to_json(&self.settings),
            _ => Ok(()),
        };
        match kind {
            "stringify" => {
                self.doc.stringify(&self.settings);
                self.sync_editors_from_doc();
            }
            "unescape" => {
                self.doc.unescape(&self.settings);
                self.sync_editors_from_doc();
            }
            _ => {}
        }
        match res {
            Ok(()) => {
                if kind == "format" || kind == "minify" || kind == "j2p" || kind == "p2j" {
                    self.sync_editors_from_doc();
                }
            }
            Err(e) => self.toast(e),
        }
    }

    fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json", "txt"])
            .add_filter("All Files", &["*"])
            .pick_file()
        {
            match self.doc.load_file(&path, &self.settings) {
                Ok(()) => {
                    self.sync_editors_from_doc();
                    self.toast(format!("Opened {}", path.display()));
                }
                Err(e) => self.toast(e),
            }
        }
    }

    fn save_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .set_file_name("untitled.json")
            .save_file()
        {
            match self.doc.save_file(&path) {
                Ok(()) => self.toast(format!("Saved {}", path.display())),
                Err(e) => self.toast(e),
            }
        }
    }

    fn paste_from_clipboard(&mut self) {
        if let Some(text) = clipboard::get_text() {
            let flag = self.doc.paste(&text, &self.settings);
            self.sync_editors_from_doc();
            if flag == "converted-python" {
                self.toast("Pasted Python dict — converted to JSON");
            } else {
                self.toast("Pasted");
            }
        } else {
            self.toast("Clipboard is empty");
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let ctrl = ctx.input(|i| i.modifiers.ctrl);
        let shift = ctx.input(|i| i.modifiers.shift);
        let alt = ctx.input(|i| i.modifiers.alt);
        let editing = ctx.wants_keyboard_input();

        // Ctrl+1/2/3 — tabs (matches macOS Cmd+1/2/3)
        if ctrl && !alt {
            if ctx.input(|i| i.key_pressed(egui::Key::Num1)) {
                self.doc.active_tab = AppTab::Viewer;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Num2)) {
                self.doc.active_tab = AppTab::Text;
                self.editor_text = self.doc.raw_text.clone();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Num3)) {
                self.doc.active_tab = AppTab::Split;
                self.split_text = self.doc.raw_text.clone();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::E)) {
                if shift {
                    self.doc.collapse_all();
                } else {
                    self.doc.expand_all();
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::F)) {
                self.show_search = true;
                self.focus_search = true;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::G)) {
                if shift {
                    self.do_search_prev();
                } else {
                    self.do_search_next();
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::O)) {
                self.open_file_dialog();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::S)) {
                self.save_file_dialog();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)) {
                self.settings.font_size = (self.settings.font_size + 1.0).clamp(9.0, 24.0);
                self.save_settings();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Minus)) {
                self.settings.font_size = (self.settings.font_size - 1.0).clamp(9.0, 24.0);
                self.save_settings();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Num0)) {
                self.settings.font_size = 12.0;
                self.save_settings();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Comma)) {
                self.show_settings = true;
            }
        }

        // Ctrl+Alt+P — toggle properties panel (matches macOS Cmd+Option+P)
        if ctrl && alt && ctx.input(|i| i.key_pressed(egui::Key::P)) {
            self.show_props = !self.show_props;
        }

        // Single key shortcuts when not editing text
        if !editing {
            if ctx.input(|i| i.key_pressed(egui::Key::Slash)) {
                self.show_search = true;
                self.focus_search = true;
            }
            if shift && ctx.input(|i| i.key_pressed(egui::Key::Slash)) {
                self.show_shortcuts = true;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                if self.show_shortcuts || self.show_settings || self.show_about {
                    self.show_shortcuts = false;
                    self.show_settings = false;
                    self.show_about = false;
                } else if !self.search_input.is_empty() {
                    self.search_input.clear();
                    self.doc.clear_search();
                } else if self.show_search {
                    self.show_search = false;
                }
            }
        }
    }

    // MARK: - Top Main Header (Segmented Control & Global Actions)
    pub fn show_main_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // App Title / Branding
            ui.strong(egui::RichText::new("{ } JSON Viewer").size(14.0));

            ui.add_space(12.0);

            // Right-aligned global actions first
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Settings
                if ui
                    .button(egui::RichText::new("Settings").size(11.0))
                    .on_hover_text("Settings (Ctrl+,)")
                    .clicked()
                {
                    self.show_settings = true;
                }

                // Shortcuts
                if ui
                    .button(egui::RichText::new("?").size(11.0).strong())
                    .on_hover_text("Keyboard Shortcuts (?)")
                    .clicked()
                {
                    self.show_shortcuts = true;
                }

                // Find
                let search_active = self.show_search;
                let search_btn = egui::Button::new(
                    egui::RichText::new("Find")
                        .size(11.0)
                        .strong()
                        .color(if search_active {
                            ui.visuals().strong_text_color()
                        } else {
                            ui.visuals().text_color()
                        }),
                )
                .fill(if search_active {
                    ui.visuals().selection.bg_fill
                } else {
                    ui.visuals().widgets.inactive.bg_fill
                });
                if ui
                    .add(search_btn)
                    .on_hover_text("Toggle & Focus Search Bar (/ or Ctrl+F)")
                    .clicked()
                {
                    self.show_search = !self.show_search;
                    if self.show_search {
                        self.focus_search = true;
                    }
                }

                // Properties Panel Toggle
                let props_active = self.show_props;
                let props_btn = egui::Button::new(
                    egui::RichText::new("Props")
                        .size(11.0)
                        .strong()
                        .color(if props_active {
                            ui.visuals().strong_text_color()
                        } else {
                            ui.visuals().text_color()
                        }),
                )
                .fill(if props_active {
                    ui.visuals().selection.bg_fill
                } else {
                    ui.visuals().widgets.inactive.bg_fill
                });
                if ui
                    .add(props_btn)
                    .on_hover_text("Toggle Properties Panel (Ctrl+Alt+P)")
                    .clicked()
                {
                    self.show_props = !self.show_props;
                }

                ui.separator();

                // Centered Segmented Tab Picker (macOS parity)
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    egui::Frame::group(ui.style())
                        .fill(ui.visuals().extreme_bg_color)
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(3, 3))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            let tabs = [
                                (AppTab::Viewer, "Viewer"),
                                (AppTab::Text, "Text"),
                                (AppTab::Split, "Split"),
                            ];
                            for (tab, label) in tabs {
                                let is_active = self.doc.active_tab == tab;
                                let btn = egui::Button::new(
                                    egui::RichText::new(label)
                                        .size(12.0)
                                        .strong()
                                        .color(if is_active {
                                            ui.visuals().strong_text_color()
                                        } else {
                                            ui.visuals().text_color()
                                        }),
                                )
                                .fill(if is_active {
                                    ui.visuals().selection.bg_fill
                                } else {
                                    egui::Color32::TRANSPARENT
                                })
                                .corner_radius(4.0)
                                .min_size(egui::vec2(68.0, 22.0));

                                if ui.add(btn).clicked() {
                                    self.doc.active_tab = tab;
                                }
                            }
                        });
                });
            });
        });
    }

    // MARK: - Viewer Tab Tree Action Bar (matching TreeViewer.swift)
    pub fn show_tree_toolbar(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button(egui::RichText::new("[+] Expand All").size(11.0)).clicked() {
                self.doc.expand_all();
            }
            if ui.button(egui::RichText::new("[-] Collapse All").size(11.0)).clicked() {
                self.doc.collapse_all();
            }

            ui.separator();

            // Copy Dropdown with 4 formats
            egui::ComboBox::from_id_salt("tree_copy_combo")
                .selected_text(egui::RichText::new("Copy ▾").size(11.0))
                .show_ui(ui, |ui| {
                    if ui.button("Copy Beautified JSON").clicked() {
                        let t = self.doc.copy_beautified(&self.settings);
                        self.copy_out(ctx, t, "Beautified JSON");
                    }
                    if ui.button("Copy Minified JSON").clicked() {
                        let t = self.doc.copy_minified();
                        self.copy_out(ctx, t, "Minified JSON");
                    }
                    if ui.button("Copy as Stringified JSON").clicked() {
                        let t = self.doc.copy_stringified(&self.settings);
                        self.copy_out(ctx, t, "Stringified JSON");
                    }
                    if ui.button("Copy as Python Dictionary").clicked() {
                        let t = self.doc.copy_python(&self.settings);
                        self.copy_out(ctx, t, "Python Dictionary");
                    }
                });

            ui.separator();

            // Zoom controls
            if ui
                .button(egui::RichText::new("Zoom -").size(11.0))
                .on_hover_text("Zoom Out (Ctrl -)")
                .clicked()
            {
                self.settings.font_size = (self.settings.font_size - 1.0).clamp(9.0, 24.0);
                self.save_settings();
            }
            if ui
                .button(egui::RichText::new("Zoom +").size(11.0))
                .on_hover_text("Zoom In (Ctrl +)")
                .clicked()
            {
                self.settings.font_size = (self.settings.font_size + 1.0).clamp(9.0, 24.0);
                self.save_settings();
            }

            ui.separator();

            // Current selected path (macOS parity)
            if let Some(path) = &self.doc.selected_path {
                ui.label(
                    egui::RichText::new(path)
                        .monospace()
                        .size(11.0)
                        .color(ui.visuals().weak_text_color()),
                );
            }
        });
    }

    // MARK: - Text Tab Action Toolbar (matching TextEditorView.swift)
    pub fn show_text_toolbar(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button(egui::RichText::new("Paste").size(11.0)).clicked() {
                self.paste_from_clipboard();
            }

            egui::ComboBox::from_id_salt("text_copy_combo")
                .selected_text(egui::RichText::new("Copy ▾").size(11.0))
                .show_ui(ui, |ui| {
                    if ui.button("Copy Raw Text").clicked() {
                        let t = self.doc.copy_text();
                        self.copy_out(ctx, t, "Raw text");
                    }
                    if ui.button("Copy Beautified JSON").clicked() {
                        let t = self.doc.copy_beautified(&self.settings);
                        self.copy_out(ctx, t, "Beautified JSON");
                    }
                    if ui.button("Copy Minified JSON").clicked() {
                        let t = self.doc.copy_minified();
                        self.copy_out(ctx, t, "Minified JSON");
                    }
                    if ui.button("Copy as Stringified JSON").clicked() {
                        let t = self.doc.copy_stringified(&self.settings);
                        self.copy_out(ctx, t, "Stringified JSON");
                    }
                    if ui.button("Copy as Python Dictionary").clicked() {
                        let t = self.doc.copy_python(&self.settings);
                        self.copy_out(ctx, t, "Python Dictionary");
                    }
                });

            if ui.button(egui::RichText::new("Clear").size(11.0)).clicked() {
                self.doc.clear();
                self.sync_editors_from_doc();
            }

            ui.separator();

            // Transform actions
            if ui
                .button(egui::RichText::new("Format").size(11.0))
                .on_hover_text("Format JSON with configured indentation")
                .clicked()
            {
                self.apply_transform("format");
            }
            if ui
                .button(egui::RichText::new("Minify").size(11.0))
                .on_hover_text("Minify JSON into single line")
                .clicked()
            {
                self.apply_transform("minify");
            }
            if ui
                .button(egui::RichText::new("Stringify").size(11.0))
                .on_hover_text("Stringify JSON with escaped slashes and quotes")
                .clicked()
            {
                self.apply_transform("stringify");
            }
            if ui
                .button(egui::RichText::new("Unescape").size(11.0))
                .on_hover_text("Unescape stringified JSON back to formatted JSON")
                .clicked()
            {
                self.apply_transform("unescape");
            }

            ui.separator();

            if ui
                .button(egui::RichText::new("JSON → Py").size(11.0))
                .on_hover_text("Convert JSON in editor directly into Python dictionary format")
                .clicked()
            {
                self.apply_transform("j2p");
            }
            if ui
                .button(egui::RichText::new("Py → JSON").size(11.0))
                .on_hover_text("Convert Python dictionary in editor directly into valid JSON")
                .clicked()
            {
                self.apply_transform("p2j");
            }

            ui.separator();

            if ui
                .button(egui::RichText::new("Open").size(11.0))
                .on_hover_text("Open JSON File (Ctrl+O)")
                .clicked()
            {
                self.open_file_dialog();
            }
            if ui
                .button(egui::RichText::new("Save").size(11.0))
                .on_hover_text("Save JSON File (Ctrl+S)")
                .clicked()
            {
                self.save_file_dialog();
            }

            ui.separator();

            if ui
                .button(egui::RichText::new("Zoom -").size(11.0))
                .on_hover_text("Zoom Out (Ctrl -)")
                .clicked()
            {
                self.settings.font_size = (self.settings.font_size - 1.0).clamp(9.0, 24.0);
                self.save_settings();
            }
            if ui
                .button(egui::RichText::new("Zoom +").size(11.0))
                .on_hover_text("Zoom In (Ctrl +)")
                .clicked()
            {
                self.settings.font_size = (self.settings.font_size + 1.0).clamp(9.0, 24.0);
                self.save_settings();
            }
        });
    }

    // MARK: - Bottom-Docked Search Bar (matching SearchToolbar.swift)
    pub fn show_search_toolbar(&mut self, ui: &mut egui::Ui) {
        if !self.show_search {
            return;
        }

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Find:").size(12.0).strong());

            let text_edit = egui::TextEdit::singleline(&mut self.search_input)
                .hint_text("Search keys, values, paths...")
                .desired_width(220.0);

            let resp = ui.add(text_edit);
            if self.focus_search {
                resp.request_focus();
                self.focus_search = false;
            }

            // Fix search on Enter:
            // Singleline TextEdit loses focus when Enter is pressed in egui.
            // Check both lost_focus and has_focus, and retain focus so consecutive Enters cycle!
            let enter_pressed = (resp.lost_focus() || resp.has_focus())
                && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter_pressed {
                let shift = ui.input(|i| i.modifiers.shift);
                if shift {
                    self.do_search_prev();
                } else if self.doc.last_executed_query != self.search_input.trim() {
                    self.do_search_go();
                } else {
                    self.do_search_next();
                }
                resp.request_focus();
            }

            let escape_pressed = resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape));
            if escape_pressed {
                if !self.search_input.is_empty() {
                    self.search_input.clear();
                    self.doc.clear_search();
                } else {
                    self.show_search = false;
                }
            }

            if !self.search_input.is_empty() {
                if ui.small_button("✕").on_hover_text("Clear search").clicked() {
                    self.search_input.clear();
                    self.doc.clear_search();
                }
            }

            // Fix GO! button:
            // If the query is unchanged and results exist, advance to next match!
            // If new query, start search.
            let go_btn = egui::Button::new(egui::RichText::new("GO!").size(11.0).strong())
                .fill(ui.visuals().selection.bg_fill);
            if ui.add(go_btn).clicked() {
                if self.doc.last_executed_query == self.search_input.trim() && !self.doc.search_results.is_empty() {
                    self.do_search_next();
                } else {
                    self.do_search_go();
                }
                resp.request_focus();
            }

            if !self.doc.search_status.is_empty() {
                let is_error = self.doc.search_status == "Phrase not found!";
                ui.label(
                    egui::RichText::new(&self.doc.search_status)
                        .size(11.0)
                        .color(if is_error {
                            egui::Color32::from_rgb(230, 80, 80)
                        } else {
                            ui.visuals().strong_text_color()
                        }),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(egui::RichText::new("✕").size(11.0))
                    .on_hover_text("Close Search Bar (Esc)")
                    .clicked()
                {
                    self.show_search = false;
                }

                let next_btn = ui
                    .button(egui::RichText::new("Next ↓").size(11.0))
                    .on_hover_text("Next Match (Enter or Ctrl+G)");
                if next_btn.clicked() {
                    self.do_search_next();
                }

                let prev_btn = ui
                    .button(egui::RichText::new("Prev ↑").size(11.0))
                    .on_hover_text("Previous Match (Shift+Enter or Ctrl+Shift+G)");
                if prev_btn.clicked() {
                    self.do_search_prev();
                }
            });
        });
    }

    // MARK: - Virtualized Tree View (matching TreeViewer.swift)
    pub fn show_tree_view(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        if self.doc.root.is_none() {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.label(egui::RichText::new("{ }").size(48.0).weak());
                ui.label("No JSON tree loaded");
                ui.add_space(8.0);
                if ui
                    .button(egui::RichText::new("Parse & View JSON").strong())
                    .clicked()
                {
                    let s = self.settings.clone();
                    self.doc.parse_and_build_tree(false, &s);
                }
            });
            return;
        }

        let total_rows = self.doc.visible_tree_rows.len();
        let row_height = (self.settings.font_size as f32 + 10.0).max(22.0);

        let mut toggle_path: Option<String> = None;
        let mut toggle_leaf_path: Option<String> = None;
        let mut select_path: Option<String> = None;
        let mut copy_payload: Option<(String, String)> = None;
        let mut expand_subtree_path: Option<String> = None;
        let mut collapse_subtree_path: Option<String> = None;

        let mut scroll_area = egui::ScrollArea::both().auto_shrink([false, false]);

        // When a search match or node navigation reveals a row, center it in the viewport once:
        if let Some(target_path) = self.doc.requested_scroll_path.take() {
            if let Some(idx) = self.doc.visible_tree_rows.iter().position(|r| r.path == target_path) {
                let available_height = ui.available_height();
                let viewport_h = if available_height.is_finite() && available_height > 60.0 {
                    available_height
                } else {
                    400.0
                };
                let row_pitch = row_height + ui.spacing().item_spacing.y;
                let target_y = idx as f32 * row_pitch;
                let target_offset = (target_y - (viewport_h - row_height) * 0.5).max(0.0);
                scroll_area = scroll_area.vertical_scroll_offset(target_offset);
            }
        }

        // Always virtualize rows so even 100,000+ line documents remain 60fps and never crash/OOM
        scroll_area.show_rows(ui, row_height, total_rows, |ui, row_range| {
            for i in row_range {
                if let Some(row) = self.doc.visible_tree_rows.get(i).cloned() {
                    render_tree_row(
                        ui,
                        &row,
                        self.doc.selected_path.as_deref(),
                        self.settings.font_size as f32,
                        &mut toggle_path,
                        &mut toggle_leaf_path,
                        &mut select_path,
                        &mut copy_payload,
                        &mut expand_subtree_path,
                        &mut collapse_subtree_path,
                    );
                }
            }
        });

        // Apply any pending interactions
        if let Some(p) = toggle_path {
            self.doc.toggle_expand(&p);
        }
        if let Some(p) = toggle_leaf_path {
            self.doc.toggle_expand_leaf(&p);
        }
        if let Some(p) = select_path {
            self.doc.selected_path = Some(p);
        }
        if let Some(p) = expand_subtree_path {
            self.doc.expand_subtree(&p);
        }
        if let Some(p) = collapse_subtree_path {
            self.doc.collapse_subtree(&p);
        }
        if let Some((label, text)) = copy_payload {
            self.copy_out(ctx, text, &label);
        }
    }

    // MARK: - Property Grid (matching PropertyGridView.swift)
    pub fn show_property_grid(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let (parent_info, properties) = self.doc.properties_for_selected();

        // Header Bar
        ui.horizontal(|ui| {
            ui.strong(egui::RichText::new("Properties").size(12.0));

            // Selected node key
            if let Some(sel) = &self.doc.selected_path {
                let name = sel.rsplit('.').next().unwrap_or(sel.as_str());
                ui.label(
                    egui::RichText::new(if name == "$" { "JSON (Root)" } else { name })
                        .monospace()
                        .size(11.0),
                );
            }

            // Jump back to parent button (macOS parity)
            if let Some((parent_path, parent_key)) = parent_info {
                let label = if parent_key == "JSON" || parent_path == "$" {
                    "← Root"
                } else {
                    "← Parent"
                };
                if ui
                    .button(egui::RichText::new(label).size(10.0))
                    .on_hover_text(format!("Jump back to {}", parent_path))
                    .clicked()
                {
                    self.doc.navigate_to_parent();
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("✕").clicked() {
                    self.show_props = false;
                }
                ui.label(
                    egui::RichText::new(format!("{} items", properties.len()))
                        .size(11.0)
                        .weak(),
                );
            });
        });

        ui.separator();

        // Property search filter
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Filter:").size(11.0));
            let filter_w = (ui.available_width() - 36.0).max(60.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.prop_filter)
                    .hint_text("Filter properties...")
                    .desired_width(filter_w),
            );
            if !self.prop_filter.is_empty() {
                if ui.small_button("✕").clicked() {
                    self.prop_filter.clear();
                }
            }
        });

        ui.separator();

        // Filter rows
        let q = self.prop_filter.to_lowercase();
        let filtered: Vec<&PropertyRow> = properties
            .iter()
            .filter(|r| {
                q.is_empty()
                    || r.name.to_lowercase().contains(&q)
                    || r.value.to_lowercase().contains(&q)
                    || r.typ.to_lowercase().contains(&q)
            })
            .collect();

        if filtered.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label("No properties match filter.");
            });
            return;
        }

        let total_count = filtered.len();
        let display_limit = 500usize;
        let display_slice = if total_count > display_limit {
            &filtered[..display_limit]
        } else {
            &filtered[..]
        };

        let mut jump_target: Option<String> = None;
        let mut copy_target: Option<(String, String)> = None;

        // Two-column table: Name | Value
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("property_grid_table")
                    .striped(true)
                    .min_col_width(80.0)
                    .show(ui, |ui| {
                        ui.strong("Name");
                        ui.strong("Value");
                        ui.end_row();

                        for row in display_slice {
                            // Clickable Name jumps to element in Tree
                            let mut name_btn = ui.selectable_label(
                                false,
                                egui::RichText::new(&row.name).monospace().strong(),
                            );
                            if name_btn.clicked() {
                                jump_target = Some(row.path.clone());
                            }
                            name_btn = name_btn.on_hover_text(format!("Click to jump to {} in Tree", row.path));

                            name_btn.context_menu(|ui| {
                                if ui.button("Go to Element in Tree").clicked() {
                                    jump_target = Some(row.path.clone());
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("Copy Value").clicked() {
                                    copy_target = Some(("Value".into(), row.value.clone()));
                                    ui.close();
                                }
                                if ui.button("Copy Name").clicked() {
                                    copy_target = Some(("Name".into(), row.name.clone()));
                                    ui.close();
                                }
                                if ui.button("Copy JSON Path").clicked() {
                                    copy_target = Some(("JSON Path".into(), row.path.clone()));
                                    ui.close();
                                }
                            });

                            // Value display with color
                            let val_label = ui.label(
                                egui::RichText::new(&row.value)
                                    .monospace()
                                    .color(type_color_by_name(&row.typ)),
                            );
                            if val_label.double_clicked() {
                                copy_target = Some(("Value".into(), row.value.clone()));
                            }

                            ui.end_row();
                        }
                    });

                if total_count > display_limit {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "Showing first {} of {} items. Use Filter to narrow down.",
                            display_limit, total_count
                        ))
                        .size(11.0)
                        .weak(),
                    );
                }
            });

        if let Some(target) = jump_target {
            self.doc.navigate_to_property(&target);
        }
        if let Some((label, val)) = copy_target {
            self.copy_out(ctx, val, &label);
        }
    }

    // MARK: - Modals (Settings, Shortcuts, About)
    pub fn show_settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let mut open = true;
        let mut save = false;

        egui::Window::new("Settings")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;

                ui.horizontal(|ui| {
                    ui.label("Indentation:");
                    egui::ComboBox::from_id_salt("settings_indent")
                        .selected_text(self.settings.indent_label())
                        .show_ui(ui, |ui| {
                            if ui.selectable_value(&mut self.settings.indent_spaces, 2, "2 Spaces (Default)").clicked() { save = true; }
                            if ui.selectable_value(&mut self.settings.indent_spaces, 4, "4 Spaces").clicked() { save = true; }
                            if ui.selectable_value(&mut self.settings.indent_spaces, -1, "Tabs").clicked() { save = true; }
                        });
                });

                if ui.checkbox(&mut self.settings.sort_keys_alphabetically, "Sort object keys alphabetically").changed() { save = true; }
                if ui.checkbox(&mut self.settings.escape_slashes_in_stringify, "Escape forward slashes (\\/) when stringifying").changed() { save = true; }
                if ui.checkbox(&mut self.settings.auto_unwrap_stringified, "Auto-unwrap stringified JSON").changed() { save = true; }
                if ui.checkbox(&mut self.settings.wrap_lines, "Wrap long lines in editor").changed() { save = true; }

                ui.horizontal(|ui| {
                    ui.label("Default Tab:");
                    egui::ComboBox::from_id_salt("settings_default_tab")
                        .selected_text(&self.settings.default_tab)
                        .show_ui(ui, |ui| {
                            if ui.selectable_value(&mut self.settings.default_tab, "Viewer".into(), "Tree Viewer").clicked() { save = true; }
                            if ui.selectable_value(&mut self.settings.default_tab, "Text".into(), "Text Editor").clicked() { save = true; }
                            if ui.selectable_value(&mut self.settings.default_tab, "Split".into(), "Split View").clicked() { save = true; }
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("Font Size:");
                    let mut sz = self.settings.font_size as i32;
                    if ui.add(egui::Slider::new(&mut sz, 9..=24)).changed() {
                        self.settings.font_size = sz as f64;
                        save = true;
                    }
                });

                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("Reset to Defaults").clicked() {
                        self.settings.reset_to_defaults();
                        save = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Done").clicked() {
                            self.show_settings = false;
                        }
                    });
                });

                ui.small(format!("Config: {}", config_path().display()));
            });

        if !open {
            self.show_settings = false;
        }
        if save {
            self.settings = self.settings.clamped();
            self.save_settings();
        }
    }

    pub fn show_shortcuts_window(&mut self, ctx: &egui::Context) {
        if !self.show_shortcuts {
            return;
        }
        let mut open = true;

        egui::Window::new("Keyboard Shortcuts")
            .open(&mut open)
            .resizable(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.shortcut_filter)
                        .hint_text("Filter shortcuts…")
                        .desired_width(ui.available_width()),
                );

                ui.add_space(6.0);

                let all: &[(&str, &str)] = &[
                    ("?", "Show shortcuts cheatsheet"),
                    ("/", "Focus search bar"),
                    ("Ctrl+F", "Focus search bar"),
                    ("Enter / Shift+Enter", "Next / previous search match"),
                    ("Ctrl+G / Ctrl+Shift+G", "Global next / previous match"),
                    ("Ctrl+1", "Viewer Tab"),
                    ("Ctrl+2", "Text Tab"),
                    ("Ctrl+3", "Split Tab"),
                    ("Ctrl+Alt+P", "Toggle Properties panel"),
                    ("Ctrl+E", "Expand all nodes"),
                    ("Ctrl+Shift+E", "Collapse all nodes"),
                    ("Ctrl+O", "Open JSON file"),
                    ("Ctrl+S", "Save JSON file"),
                    ("Ctrl+Plus / Ctrl+Minus", "Zoom in / Zoom out"),
                    ("Ctrl+0", "Reset zoom (12pt)"),
                    ("Ctrl+,", "Open settings"),
                    ("Esc", "Close dialog / clear search"),
                ];

                let q = self.shortcut_filter.to_lowercase();
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    egui::Grid::new("shortcuts_grid_modal")
                        .striped(true)
                        .show(ui, |ui| {
                            for (k, a) in all {
                                if !q.is_empty()
                                    && !k.to_lowercase().contains(&q)
                                    && !a.to_lowercase().contains(&q)
                                {
                                    continue;
                                }
                                ui.monospace(*k);
                                ui.label(*a);
                                ui.end_row();
                            }
                        });
                });
            });

        if !open {
            self.show_shortcuts = false;
        }
    }

    pub fn show_about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut open = true;

        egui::Window::new("About JSON Viewer")
            .open(&mut open)
            .resizable(false)
            .default_width(340.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.strong(egui::RichText::new("{ } JSON Viewer").size(18.0));
                    ui.add_space(4.0);
                    ui.label("Version 1.0.0 (Linux)");
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Fast, lightweight JSON viewer, formatter & inspection tool for Linux Wayland/X11.")
                            .size(11.0)
                            .weak(),
                    );
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);
                    ui.label("Developed for Omarchy Linux & cross-distro Linux.");
                    ui.label("Full parity with macOS SwiftUI version.");
                    ui.add_space(12.0);
                    if ui.button("Close").clicked() {
                        self.show_about = false;
                    }
                });
            });

        if !open {
            self.show_about = false;
        }
    }
}

// MARK: - Tree Row Renderer
#[allow(clippy::too_many_arguments)]
fn render_tree_row(
    ui: &mut egui::Ui,
    row: &FlatTreeRow,
    selected_path: Option<&str>,
    font_size: f32,
    toggle_path: &mut Option<String>,
    toggle_leaf_path: &mut Option<String>,
    select_path: &mut Option<String>,
    copy_payload: &mut Option<(String, String)>,
    expand_subtree_path: &mut Option<String>,
    collapse_subtree_path: &mut Option<String>,
) {
    let is_selected = selected_path == Some(row.path.as_str());

    ui.horizontal(|ui| {
        // Indentation
        if row.depth > 0 {
            ui.add_space(row.depth as f32 * 16.0);
        }

        // Disclosure toggle button
        if row.is_container {
            let symbol = if row.is_expanded { "[-] " } else { "[+] " };
            let toggle_btn = ui
                .button(egui::RichText::new(symbol).monospace().size(font_size).strong())
                .on_hover_text(if row.is_expanded { "Collapse" } else { "Expand" });
            if toggle_btn.clicked() {
                *toggle_path = Some(row.path.clone());
            }
        } else if row.full_str.is_some() && row.char_count > 45 {
            // Big text toggle
            let symbol = if row.is_leaf_expanded { "[-]" } else { "[+]" };
            let toggle_btn = ui
                .button(
                    egui::RichText::new(symbol)
                        .monospace()
                        .size((font_size - 2.0).max(9.0))
                        .color(ui.visuals().selection.bg_fill),
                )
                .on_hover_text(if row.is_leaf_expanded { "Collapse text" } else { "Expand text" });
            if toggle_btn.clicked() {
                *toggle_leaf_path = Some(row.path.clone());
            }
        } else {
            ui.add_space(20.0);
        }

        // Type badge (str, num, bool, null, {N}, [N])
        let badge_color = type_color_by_name(row.type_name);
        egui::Frame::group(ui.style())
            .fill(badge_color.gamma_multiply(0.18))
            .corner_radius(3.0)
            .inner_margin(egui::Margin::symmetric(3, 1))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(&row.badge_text)
                        .monospace()
                        .size((font_size - 2.0).max(9.0))
                        .color(badge_color)
                        .strong(),
                );
            });

        // Key & value
        let key_text = if row.key == "JSON" { "JSON" } else { &row.key };
        let row_resp = if row.is_container {
            ui.selectable_label(
                is_selected,
                egui::RichText::new(key_text).monospace().size(font_size).strong(),
            )
        } else {
            let label = format!("{} : {}", key_text, row.value_repr);
            ui.selectable_label(
                is_selected,
                egui::RichText::new(label)
                    .monospace()
                    .size(font_size)
                    .color(badge_color),
            )
        };

        if row.is_match {
            ui.add_space(4.0);
            egui::Frame::group(ui.style())
                .fill(ui.visuals().selection.bg_fill)
                .corner_radius(3.0)
                .inner_margin(egui::Margin::symmetric(4, 1))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("MATCH")
                            .monospace()
                            .size((font_size - 3.0).max(8.0))
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                });
        }

        if row_resp.clicked() {
            *select_path = Some(row.path.clone());
        }
        if row_resp.double_clicked() {
            if row.is_container {
                *toggle_path = Some(row.path.clone());
            } else if row.full_str.is_some() && row.char_count > 45 {
                *toggle_leaf_path = Some(row.path.clone());
            }
        }

        // Context menu
        let path = row.path.clone();
        let key = row.key.clone();
        let value_repr = row.value_repr.clone();
        let full_str = row.full_str.clone();

        row_resp.context_menu(|ui| {
            if row.is_container {
                if ui.button("Expand All Sub-levels").clicked() {
                    *expand_subtree_path = Some(path.clone());
                    ui.close();
                }
                if ui.button("Collapse All Sub-levels").clicked() {
                    *collapse_subtree_path = Some(path.clone());
                    ui.close();
                }
                ui.separator();
            }
            if ui.button("Copy Value").clicked() {
                let v = full_str.clone().unwrap_or(value_repr.clone());
                *copy_payload = Some(("Value".into(), v));
                ui.close();
            }
            if ui.button("Copy Key").clicked() {
                *copy_payload = Some(("Key".into(), key.clone()));
                ui.close();
            }
            if ui.button("Copy JSON Path").clicked() {
                *copy_payload = Some(("JSON Path".into(), path.clone()));
                ui.close();
            }
        });

        // Search MATCH badge
        if row.is_match {
            egui::Frame::group(ui.style())
                .fill(ui.visuals().selection.bg_fill)
                .corner_radius(3.0)
                .inner_margin(egui::Margin::symmetric(4, 1))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("MATCH")
                            .size(9.0)
                            .strong()
                            .color(egui::Color32::WHITE),
                    );
                });
        }
    });

    // If inline big text is expanded: render multi-line box with copy button
    if row.is_leaf_expanded {
        if let Some(s) = &row.full_str {
            ui.horizontal(|ui| {
                ui.add_space((row.depth as f32 * 16.0) + 36.0);
                egui::Frame::group(ui.style())
                    .fill(ui.visuals().extreme_bg_color)
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.small(format!("{} chars", row.char_count));
                            if ui.small_button("Copy").clicked() {
                                *copy_payload = Some(("Value".into(), s.clone()));
                            }
                            if ui.small_button("Collapse").clicked() {
                                *toggle_leaf_path = Some(row.path.clone());
                            }
                        });
                        ui.add(
                            egui::TextEdit::multiline(&mut s.clone())
                                .desired_width(f32::INFINITY)
                                .font(egui::FontId::monospace(font_size)),
                        );
                    });
            });
        }
    }
}

fn type_color_by_name(t: &str) -> egui::Color32 {
    match t {
        "string" | "str" => egui::Color32::from_rgb(90, 160, 255),   // Blue
        "number" | "num" => egui::Color32::from_rgb(90, 205, 125),   // Green
        "boolean" | "bool" => egui::Color32::from_rgb(235, 195, 75), // Amber
        "null" => egui::Color32::from_rgb(235, 90, 90),              // Red
        "object" => egui::Color32::from_rgb(185, 130, 255),          // Purple
        "array" => egui::Color32::from_rgb(140, 140, 255),           // Indigo
        _ => egui::Color32::from_rgb(170, 170, 170),
    }
}

fn config_path() -> std::path::PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return std::path::PathBuf::from(xdg).join("JSONViewer").join("config.json");
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".config").join("JSONViewer").join("config.json")
}

// MARK: - eframe App Implementation
impl eframe::App for ViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.initialized {
            self.initialized = true;
            self.doc.active_tab = match self.settings.default_tab.as_str() {
                "Viewer" => AppTab::Viewer,
                "Split" => AppTab::Split,
                _ => AppTab::Text,
            };
            ctx.request_repaint();
        }

        // Handle dropped files from file manager (Nautilus, Dolphin, etc.)
        ctx.input(|i| {
            if let Some(file) = i.raw.dropped_files.first() {
                if let Some(path) = &file.path {
                    if let Ok(()) = self.doc.load_file(path, &self.settings) {
                        self.sync_editors_from_doc();
                        self.toast(format!("Opened {}", path.display()));
                    }
                }
            }
        });

        self.handle_shortcuts(ctx);

        // Toast timer
        if let Some((_, t)) = &self.toast {
            if t.elapsed().as_secs() > 3 {
                self.toast = None;
            }
        }

        // Top Main Header
        egui::TopBottomPanel::top("top_header")
            .exact_height(38.0)
            .show(ctx, |ui| {
                self.show_main_header(ui);
            });

        // Bottom Status Bar
        egui::TopBottomPanel::bottom("bottom_statusbar")
            .exact_height(26.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let status = self.doc.status_text();
                    let is_err = self.doc.parse_error.is_some();
                    ui.label(
                        egui::RichText::new(&status)
                            .size(11.0)
                            .color(if is_err {
                                egui::Color32::from_rgb(230, 80, 80)
                            } else {
                                egui::Color32::from_rgb(90, 200, 120)
                            })
                            .strong(),
                    );
                    ui.separator();
                    ui.label(
                        egui::RichText::new(self.doc.metrics_text())
                            .size(11.0)
                            .weak(),
                    );
                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!("Zoom: {}pt", self.settings.font_size as i32))
                            .size(11.0)
                            .weak(),
                    );

                    if let Some((msg, _)) = &self.toast {
                        ui.separator();
                        ui.strong(egui::RichText::new(msg).size(11.0).color(ui.visuals().selection.bg_fill));
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("About").clicked() {
                            self.show_about = true;
                        }
                    });
                });
            });

        // Tab synchronization
        if self.last_tab != self.doc.active_tab {
            match self.doc.active_tab {
                AppTab::Text => self.editor_text = self.doc.raw_text.clone(),
                AppTab::Split => self.split_text = self.doc.raw_text.clone(),
                AppTab::Viewer => {
                    let s = self.settings.clone();
                    self.doc.parse_and_build_tree(true, &s);
                }
            }
            self.last_tab = self.doc.active_tab;
        }

        // Central Content Panel
        egui::CentralPanel::default().show(ctx, |ui| match self.doc.active_tab {
            AppTab::Viewer => {
                // Top tree toolbar
                self.show_tree_toolbar(ctx, ui);
                ui.separator();

                // Search Toolbar (if open)
                if self.show_search {
                    self.show_search_toolbar(ui);
                    ui.separator();
                }

                // Main Viewer area (Tree on left, Property Grid on right if open)
                if self.show_props {
                    ui.columns(2, |cols| {
                        cols[0].group(|ui| {
                            self.show_tree_view(ctx, ui);
                        });
                        cols[1].group(|ui| {
                            self.show_property_grid(ctx, ui);
                        });
                    });
                } else {
                    self.show_tree_view(ctx, ui);
                }
            }

            AppTab::Text => {
                self.show_text_toolbar(ctx, ui);
                ui.separator();

                let font = egui::FontId::monospace(self.settings.font_size as f32);
                let text_edit = egui::TextEdit::multiline(&mut self.editor_text)
                    .code_editor()
                    .desired_rows(30)
                    .desired_width(f32::INFINITY)
                    .font(font);

                let resp = egui::ScrollArea::both().show(ui, |ui| ui.add(text_edit)).inner;

                if resp.changed() {
                    self.doc.raw_text = self.editor_text.clone();
                    self.doc.mark_edited();
                    self.last_edit = Instant::now();
                    self.pending_reparse = true;
                }

                // Debounced re-parsing (350ms)
                if self.pending_reparse && self.last_edit.elapsed().as_millis() > 350 {
                    let s = self.settings.clone();
                    let _ = self.doc.parse_and_build_tree(true, &s);
                    self.pending_reparse = false;
                    ctx.request_repaint();
                }
            }

            AppTab::Split => {
                ui.columns(2, |cols| {
                    // Left: Live Text Editor with text toolbar
                    {
                        let left = &mut cols[0];
                        self.show_text_toolbar(ctx, left);
                        left.separator();

                        let font = egui::FontId::monospace(self.settings.font_size as f32);
                        let text_edit = egui::TextEdit::multiline(&mut self.split_text)
                            .code_editor()
                            .desired_rows(30)
                            .desired_width(f32::INFINITY)
                            .font(font);

                        let resp = egui::ScrollArea::both().show(left, |ui| ui.add(text_edit)).inner;

                        if resp.changed() {
                            self.doc.raw_text = self.split_text.clone();
                            self.editor_text = self.split_text.clone();
                            self.doc.mark_edited();
                            self.last_edit = Instant::now();
                            self.pending_reparse = true;
                        }
                    }

                    // Right: Tree Viewer + Property Grid + Search Toolbar
                    {
                        let right = &mut cols[1];
                        self.show_tree_toolbar(ctx, right);
                        right.separator();

                        if self.show_search {
                            self.show_search_toolbar(right);
                            right.separator();
                        }

                        if self.show_props {
                            right.columns(2, |subcols| {
                                subcols[0].group(|ui| {
                                    self.show_tree_view(ctx, ui);
                                });
                                subcols[1].group(|ui| {
                                    self.show_property_grid(ctx, ui);
                                });
                            });
                        } else {
                            self.show_tree_view(ctx, right);
                        }
                    }
                });

                if self.pending_reparse && self.last_edit.elapsed().as_millis() > 350 {
                    let s = self.settings.clone();
                    let _ = self.doc.parse_and_build_tree(true, &s);
                    self.pending_reparse = false;
                    ctx.request_repaint();
                }
            }
        });

        self.show_settings_window(ctx);
        self.show_shortcuts_window(ctx);
        self.show_about_window(ctx);

        if self.pending_reparse {
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
        } else if self.toast.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }
}
