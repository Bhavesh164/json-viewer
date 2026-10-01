//! Throughput benchmark for the Text tab editor path.
//!
//! Run with:
//!   cargo bench --bench text_tab
//!
//! Compares the two editor implementations on the same heavy documents:
//!
//! * `egui::TextEdit::multiline` — what the Linux build used before. It shapes
//!   and wraps the whole buffer on every frame, so its cost grows linearly with
//!   file size.
//! * `jsonviewer::CodeEditor` — the egui counterpart of the macOS
//!   `NSTextStorage` + `NSTextView` editor, which only shapes the rows that
//!   intersect the viewport.
//!
//! The target is a 16 ms frame budget (60 fps).

use std::time::Instant;

use egui::{Context, Pos2, RawInput, Rect, Vec2};
use jsonviewer::code_editor::CodeEditor;
use jsonviewer::setup_fonts;

const FRAMES: usize = 10;
const SCREEN: Vec2 = Vec2::new(1400.0, 900.0);

fn build_heavy_json(records: usize) -> String {
    let mut s = String::from("{\n  \"meta\": {\n    \"count\": ");
    s.push_str(&records.to_string());
    s.push_str("\n  },\n  \"records\": [\n");
    for i in 0..records {
        if i > 0 {
            s.push_str(",\n");
        }
        s.push_str(&format!(
            "    {{\"id\": {i}, \"name\": \"name_{i}\", \"email\": \"user{i}@example.com\", \
             \"active\": {}, \"tags\": [\"t{}\", \"t{}\"], \"score\": {:.2}, \
             \"address\": {{\"city\": \"City{}\", \"zip\": \"{:05}\", \"geo\": {{\"lat\": {:.5}, \"lon\": {:.5}}}}}, \
             \"desc\": \"lorem ipsum dolor sit amet {i}\"}}",
            i % 3 == 0,
            i % 50,
            i % 37,
            i as f64 * 1.37,
            i % 400,
            i % 99999,
            -90.0 + i as f64 * 0.001,
            180.0 - i as f64 * 0.001
        ));
    }
    s.push_str("\n  ]\n}");
    s
}

fn raw_input() -> RawInput {
    let mut raw = RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, SCREEN));
    raw.viewport_id = egui::ViewportId::ROOT;
    raw
}

/// Old path: egui's own multiline text edit inside a scroll area.
fn bench_text_edit(text: &str) -> f64 {
    let ctx = Context::default();
    setup_fonts(&ctx);
    let font = egui::FontId::monospace(13.0);
    let mut buffer = text.to_string();
    let mut total = 0.0;
    for _ in 0..FRAMES {
        let raw = raw_input();
        let start = Instant::now();
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let edit = egui::TextEdit::multiline(&mut buffer)
                    .code_editor()
                    .desired_rows(30)
                    .desired_width(f32::INFINITY)
                    .font(font.clone());
                egui::ScrollArea::both()
                    .id_salt("text_editor_scroll_area")
                    .auto_shrink([false, false])
                    .show(ui, |ui| ui.add(edit));
            });
        });
        total += start.elapsed().as_secs_f64() * 1000.0;
    }
    total / FRAMES as f64
}

/// New path: the virtualized editor.
fn bench_code_editor(text: &str) -> f64 {
    let ctx = Context::default();
    setup_fonts(&ctx);
    let font = egui::FontId::monospace(13.0);
    let mut editor = CodeEditor::new(text);
    let mut total = 0.0;
    for _ in 0..FRAMES {
        let raw = raw_input();
        let start = Instant::now();
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                editor.show(ui, font.clone());
            });
        });
        total += start.elapsed().as_secs_f64() * 1000.0;
    }
    total / FRAMES as f64
}

/// Keystroke latency: one character typed, both at the top of the document and
/// after scrolling far into it.
fn bench_typing(text: &str) -> (f64, f64) {
    let ctx = Context::default();
    setup_fonts(&ctx);
    let font = egui::FontId::monospace(13.0);
    let deep_line = text.bytes().filter(|&b| b == b'\n').count() / 2;

    // Scroll the editor deep into the document with wheel events, then time the
    // frame that handles a keystroke.
    let measure = |virtualized: bool| {
        let mut editor = CodeEditor::new(text);
        let mut buffer = text.to_string();
        let draw = |ui: &mut egui::Ui| {
            if virtualized {
                editor.show(ui, font.clone());
            } else {
                let edit = egui::TextEdit::multiline(&mut buffer)
                    .code_editor()
                    .desired_rows(30)
                    .desired_width(f32::INFINITY)
                    .font(font.clone());
                egui::ScrollArea::both()
                    .id_salt("text_editor_scroll_area")
                    .auto_shrink([false, false])
                    .show(ui, |ui| ui.add(edit));
            }
        };

        // First frame: font atlas, layout caches and the line index.
        let _ = ctx.run(raw_input(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw(ui));
        });

        // Move the caret down into the middle of the document.
        for _ in 0..200 {
            let mut raw = raw_input();
            raw.events.push(egui::Event::Key {
                key: egui::Key::ArrowDown,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::default(),
            });
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| draw(ui));
            });
        }

        // Time the frame that applies one typed character.
        let mut raw = raw_input();
        raw.events.push(egui::Event::Text("x".to_string()));
        let start = Instant::now();
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw(ui));
        });
        let first = start.elapsed().as_secs_f64() * 1000.0;

        // And the frame after it, which repaints the edited document.
        let start = Instant::now();
        let _ = ctx.run(raw_input(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw(ui));
        });
        let second = start.elapsed().as_secs_f64() * 1000.0;

        let _ = deep_line;
        first.min(second)
    };

    (measure(false), measure(true))
}

fn main() {
    let mut cases: Vec<(String, String)> = Vec::new();
    for path in ["/tmp/heavy5.json", "/tmp/heavy50.json"] {
        if let Ok(t) = std::fs::read_to_string(path) {
            cases.push((path.to_string(), t));
        }
    }
    for n in [200usize, 2_000, 20_000] {
        cases.push((format!("generated {n} records"), build_heavy_json(n)));
    }
    if let Ok(t) = std::fs::read_to_string("/tmp/heavy5.json") {
        // Minified output collapses a big document onto a single very long line.
        cases.push((
            "minified 7.7 MB (1 line)".to_string(),
            jsonviewer::json::JSONParser::parse(t.trim())
                .map(|v| v.minify(false))
                .unwrap_or_default(),
        ));
    }

    println!("Text tab editor — per-frame cost ({} frames each, {SCREEN:?} viewport)", FRAMES);
    println!(
        "{:<26} {:>9} {:>9} {:>14} {:>14} {:>10}",
        "document", "size", "lines", "TextEdit", "CodeEditor", "speedup"
    );
    println!("{}", "-".repeat(90));

    for (name, text) in &cases {
        let lines = 1 + text.bytes().filter(|&b| b == b'\n').count();
        let old = bench_text_edit(text);
        let new = bench_code_editor(text);
        println!(
            "{name:<26} {:>7.1} MB {:>9} {:>11.3} ms {:>11.3} ms {:>9.0}x",
            text.len() as f64 / 1_048_576.0,
            lines,
            old,
            new,
            old / new.max(f64::MIN_POSITIVE)
        );
    }

    if let Some((_, text)) = cases.first() {
        let (old, new) = bench_typing(text);
        println!("\nkeystroke frame (one character typed mid-document, {})", cases[0].0);
        println!("  TextEdit   {old:.3} ms");
        println!("  CodeEditor {new:.3} ms");

    }
}
