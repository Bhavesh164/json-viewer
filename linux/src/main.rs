use jsonviewer::app::ViewerApp;
use jsonviewer::desktop;
use jsonviewer::settings::SettingsStore;
use jsonviewer::setup_fonts;

const HELP: &str = "\
jsonviewer - JSON Viewer and Formatter (Linux)

Usage:
  jsonviewer [FILE]        open FILE (.json) or raw JSON text
  jsonviewer --install     install .desktop launcher and icons to ~/.local/share
  jsonviewer --help        print this help

Config: $XDG_CONFIG_HOME/JSONViewer/config.json (~/.config/JSONViewer/config.json)
";

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        let msg = format!(
            "================ JSON VIEWER CRASH REPORT ================\n\
             Timestamp: {:?}\n\
             Panic Info: {}\n\
             Backtrace:\n{}\n\
             ========================================================\n",
            std::time::SystemTime::now(),
            info,
            backtrace
        );
        eprintln!("{}", msg);
        let _ = std::fs::write("/tmp/jsonviewer_crash.log", &msg);
        if let Ok(home) = std::env::var("HOME") {
            let log_dir = std::path::PathBuf::from(home).join(".local/state/jsonviewer");
            let _ = std::fs::create_dir_all(&log_dir);
            let _ = std::fs::write(log_dir.join("crash.log"), &msg);
        }
    }));
}

fn main() -> eframe::Result<()> {
    install_panic_hook();

    let pid = std::process::id();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let wayland = std::env::var("WAYLAND_DISPLAY").ok();
    let display = std::env::var("DISPLAY").ok();

    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/jsonviewer_runtime.log")
    {
        use std::io::Write;
        let _ = writeln!(
            f,
            "[{:?}] MAIN START: pid={}, args={:?}, WAYLAND={:?}, DISPLAY={:?}",
            std::time::SystemTime::now(),
            pid,
            args,
            wayland,
            display
        );
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{}", HELP);
        return Ok(());
    }
    if args.iter().any(|a| a == "--install") {
        desktop::ensure_desktop_integration();
        println!("JSON Viewer desktop entry and icons installed successfully.");
        return Ok(());
    }

    // Ensure desktop entry and icons are registered for Omarchy / XDG launchers
    desktop::ensure_desktop_integration();

    // Filter out desktop placeholder arguments like "%F", "%f", "%U", "%u"
    let initial = args
        .first()
        .filter(|a| !a.starts_with('%') && !a.trim().is_empty())
        .cloned();

    let store = SettingsStore::new();
    let settings = store.settings.clone();
    // Persist clamped defaults on first run (mirrors mouseless/linux pattern).
    store.save();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1180.0, 760.0])
        .with_min_inner_size([720.0, 480.0])
        .with_app_id("jsonviewer");

    if let Some(icon) = desktop::app_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }

    let options = eframe::NativeOptions {
        viewport,
        vsync: false,
        run_and_return: false,
        ..Default::default()
    };

    let result = eframe::run_native(
        "JSON Viewer",
        options,
        Box::new(move |cc| {
            setup_fonts(&cc.egui_ctx);
            let mut style = (*cc.egui_ctx.style()).clone();
            style.spacing.scroll.dormant_handle_opacity = 0.6;
            style.spacing.scroll.dormant_background_opacity = 0.2;
            style.spacing.scroll.floating_allocated_width = 8.0;
            style.spacing.scroll.bar_width = 8.0;
            cc.egui_ctx.set_style(style);
            Ok(Box::new(ViewerApp::new(settings, initial)))
        }),
    );

    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/jsonviewer_runtime.log")
    {
        use std::io::Write;
        let _ = writeln!(
            f,
            "[{:?}] MAIN EXIT: pid={}, result={:?}",
            std::time::SystemTime::now(),
            pid,
            result
        );
    }

    result
}
