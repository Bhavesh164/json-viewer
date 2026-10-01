//! Throughput benchmark for the Text tab editor path.
//!
//! Run with:
//!   cargo bench --bench text_tab
//!
//! Measures wall-clock time per simulated egui frame while the Text tab
//! renders a heavy document. egui's `TextEdit::multiline` lays out the whole
//! document on every frame, so this number grows linearly with file size
//! unless the editor is virtualized.

use egui::Context;
use jsonviewer::app::ViewerApp;
use jsonviewer::model::AppTab;
use jsonviewer::setup_fonts;
use jsonviewer::Settings;
use std::time::Instant;

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

fn bench_label(name: &str, bytes: usize, lines: usize, per_frame_ms: f64) {
    println!(
        "{name:<28} {:>9.2} MB  {:>8} lines  {:>9.3} ms/frame",
        bytes as f64 / 1_048_576.0,
        lines,
        per_frame_ms
    );
}

fn run_text_tab(text: &str, frames: usize) -> f64 {
    let ctx = Context::default();
    setup_fonts(&ctx);

    let mut settings = Settings::default();
    settings.default_tab = "Text".to_string();
    settings.wrap_lines = false;

    let mut app = ViewerApp::new(settings, None);
    app.editor_text = text.to_string();
    app.doc.raw_text = text.to_string();
    app.doc.update_metrics();
    app.doc.active_tab = AppTab::Text;
    app.last_tab = AppTab::Text;

    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(egui::Rect::from_min_size(
        egui::pos2(0.0, 0.0),
        egui::vec2(1400.0, 900.0),
    ));
    raw.viewport_id = egui::ViewportId::ROOT;

    let mut total = 0.0;
    for _ in 0..frames {
        let start = Instant::now();
        let _ = ctx.run(raw.clone(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.show_text_toolbar(ctx, ui);
                ui.separator();
                let font = egui::FontId::monospace(app.settings.font_size as f32);
                let mut text = std::mem::take(&mut app.editor_text);
                let text_edit = egui::TextEdit::multiline(&mut text)
                    .code_editor()
                    .desired_rows(30)
                    .desired_width(f32::INFINITY)
                    .font(font);
                egui::ScrollArea::both()
                    .id_salt("text_editor_scroll_area")
                    .auto_shrink([false, false])
                    .show(ui, |ui| ui.add(text_edit));
                app.editor_text = text;
            });
        });
        total += start.elapsed().as_secs_f64() * 1000.0;
    }
    total / frames as f64
}

fn main() {
    println!("egui Text tab editor — per-frame layout cost");
    println!("{}", "-".repeat(78));

    let mut cases: Vec<(String, &str)> = Vec::new();
    for path in ["/tmp/heavy5.json", "/tmp/heavy50.json"] {
        if let Ok(t) = std::fs::read_to_string(path) {
            cases.push((format!("{}", path), Box::leak(t.into_boxed_str())));
        }
    }
    if cases.is_empty() {
        for n in [200usize, 2_000, 20_000] {
            let t = build_heavy_json(n);
            cases.push((format!("generated {n} records"), Box::leak(t.into_boxed_str())));
        }
    }

    for (name, text) in &cases {
        let lines = 1 + text.bytes().filter(|&b| b == b'\n').count();
        let ms = run_text_tab(text, 10);
        bench_label(name, text.len(), lines, ms);
    }
}
