//! Diagnostic: compare the caret x the editor paints against where egui
//! actually places the character at the caret column.

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Vec2};
use jsonviewer::setup_fonts;

fn raw() -> egui::RawInput {
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0)));
    raw.viewport_id = egui::ViewportId::ROOT;
    raw
}

#[test]
fn diagnose_caret_offset() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);
    let font = FontId::monospace(13.0);
    let mut editor = jsonviewer::CodeEditor::new("0123456789ABCDEFGHIJ\nsecond line\n");
    // Warm-up frame: the font atlas only exists after the first run.
    let _ = ctx.run(raw(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            editor.show(ui, font.clone());
        });
    });

    // The painter's own notion of a monospace cell.
    let char_w = ctx.fonts(|f| f.glyph_width(&font, '0'));
    let row_h = ctx.fonts(|f| f.row_height(&font));
    println!("glyph_width('0') = {char_w}");
    println!("row_height        = {row_h}");

    // What egui really does when it shapes a line, and where each character
    // ends up relative to the left edge of the painted galley.
    let line = "0123456789ABCDEFGHIJ";
    let galley = ctx.fonts(|f| {
        f.layout(
            line.to_owned(),
            font.clone(),
            Color32::WHITE,
            f32::INFINITY,
        )
    });
    println!("galley rect = {:?}", galley.rect);
    for col in [0usize, 1, 5, 10, 20] {
        let r = galley.pos_from_cursor(egui::text::CCursor::new(col));
        println!(
            "  col {col:>2}: x = {:>8.4}  (col*char_w = {:>8.4})  delta = {:+.4}",
            r.min.x,
            col as f32 * char_w,
            r.min.x - col as f32 * char_w
        );
    }

    // Now run one real editor frame and pull the caret rectangle back out of
    // the shape list, then compare it with the character position.
    let out = ctx.run(raw(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            editor.show(ui, font.clone());
        });
    });

    // Find the painted text galley and the caret rect.
    let mut caret_x: Option<f32> = None;
    let mut text_left: Option<f32> = None;
    for clipped in &out.shapes {
        match &clipped.shape {
            egui::Shape::Rect(r) => {
                let r = r.rect;
                // The caret is a narrow, tall rect on the first row.
                if (r.width() - 2.0).abs() < 0.6 && r.height() > 3.0 && r.min.y < 40.0 {
                    caret_x = Some(r.min.x);
                }
            }
            egui::Shape::Mesh(m) => {
                text_left.get_or_insert(m.vertices.first().map_or(0.0, |v| v.pos.x));
            }
            _ => {}
        }
    }
    println!("caret_x = {caret_x:?}  first mesh clip x = {text_left:?}");

    // Where should it be? The editor's content origin is the panel's inner
    // rect, so recompute the same way the editor does.
    let _ = Align2::LEFT_TOP;
    let _ = Sense::click_and_drag();
}
