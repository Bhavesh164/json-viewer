//! Throwaway: time the parts of a keystroke on a large document.

use jsonviewer::code_editor::CodeEditor;
use jsonviewer::settings::Settings;
use jsonviewer::app::ViewerApp;
use jsonviewer::model::AppTab;
use jsonviewer::setup_fonts;

fn raw() -> egui::RawInput {
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(egui::Rect::from_min_size(
        egui::pos2(0.0, 0.0),
        egui::vec2(1400.0, 900.0),
    ));
    raw.viewport_id = egui::ViewportId::ROOT;
    raw
}

fn heavy(n: usize) -> String {
    let mut text = String::from("{\n  \"records\": [\n");
    for i in 0..n {
        if i > 0 {
            text.push_str(",\n");
        }
        text.push_str(&format!("    {{\"id\": {i}, \"name\": \"name_{i}\"}}}}"));
    }
    text.push_str("\n  ]\n}");
    text
}

#[test]
fn profile_keystroke() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);
    let font = egui::FontId::monospace(13.0);
    let text = heavy(20_000);
    println!("doc: {:.2} MB, {} lines", text.len() as f64 / 1e6, text.matches('\n').count() + 1);

    // 1. Raw buffer cost of one edit.
    let mut buf = CodeEditor::new(&text);
    let c = buf.buffer().cursor_of(0);
    let t = std::time::Instant::now();
    for _ in 0..20 {
        let _ = buf.replace(c..c, "q");
    }
    println!("TextBuffer::replace       {:>8.3} ms", t.elapsed().as_secs_f64() * 1000.0 / 20.0);

    // 2. Editor frame.
    let mut editor = CodeEditor::new(&text);
    let _ = ctx.run(raw(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| editor.show(ui, font.clone()));
    });
    let mut r = raw();
    r.events.push(egui::Event::Text("q".into()));
    let t = std::time::Instant::now();
    for _ in 0..10 {
        let _ = ctx.run(r.clone(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| editor.show(ui, font.clone()));
        });
    }
    println!("CodeEditor::show (typed)  {:>8.3} ms", t.elapsed().as_secs_f64() * 100.0);

    // 3. The model side: clone + metrics.
    let mut doc = jsonviewer::model::DocumentModel::new(&Settings::default());
    doc.raw_text = text.clone();
    doc.update_metrics();
    let t = std::time::Instant::now();
    for _ in 0..20 {
        let mut m = jsonviewer::model::DocumentModel::new(&Settings::default());
        m.raw_text = editor.text().to_string();
        m.mark_edited();
    }
    println!("clone + mark_edited        {:>8.3} ms", t.elapsed().as_secs_f64() * 50.0);

    // 4. Full tab.
    let mut app = ViewerApp::new(Settings::default(), None);
    app.doc.raw_text = text.clone();
    app.doc.update_metrics();
    app.doc.active_tab = AppTab::Text;
    app.last_tab = AppTab::Text;
    app.editor.set_text(&app.doc.raw_text);
    let _ = ctx.run(raw(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.show_text_tab(&ctx, ui));
    });
    let mut r = raw();
    r.events.push(egui::Event::Text("q".into()));
    let t = std::time::Instant::now();
    for _ in 0..10 {
        let _ = ctx.run(r.clone(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_text_tab(&ctx, ui));
        });
    }
    println!("show_text_tab (typed)      {:>8.3} ms", t.elapsed().as_secs_f64() * 100.0);

    // 5. A full parse, for reference.
    let t = std::time::Instant::now();
    let mut doc2 = jsonviewer::model::DocumentModel::new(&Settings::default());
    doc2.raw_text = text.clone();
    doc2.update_metrics();
    let _ = doc2.parse_and_build_tree(true, &Settings::default());
    println!("parse_and_build_tree       {:>8.3} ms (one-off)", t.elapsed().as_secs_f64() * 1000.0);
}
