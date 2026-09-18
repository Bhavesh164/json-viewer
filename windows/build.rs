use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    if !target.contains("windows") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let rc_path = manifest_dir.join("resources").join("resource.rc");
    if !rc_path.exists() {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let obj_path = out_dir.join("resource.o");

    let tool = find_windres(&target);
    let windres = match tool {
        Some(t) => t,
        None => {
            println!("cargo:warning=No windres/rc found — building without embedded icon");
            return;
        }
    };

    let result = Command::new(&windres)
        .arg(&rc_path)
        .arg("-O")
        .arg("coff")
        .arg("-o")
        .arg(&obj_path)
        .arg("-I")
        .arg(manifest_dir.join("resources"))
        .status();

    match result {
        Ok(status) if status.success() => {
            println!("cargo:rerun-if-changed={}", rc_path.display());
            println!("cargo:rustc-link-arg-bin=jsonviewer={}", obj_path.display());
        }
        _ => {
            println!("cargo:warning=windres failed — building without embedded icon");
        }
    }
}

fn find_windres(target: &str) -> Option<String> {
    let candidates: Vec<String> = if target.contains("msvc") {
        vec!["rc.exe".to_string()]
    } else {
        let prefix = target
            .replace("-pc-windows-gnu", "-w64-mingw32")
            .replace("-windows-gnu", "-w64-mingw32");
        vec![
            format!("{}-windres", prefix),
            "llvm-windres".to_string(),
            "windres".to_string(),
        ]
    };

    for c in &candidates {
        if which(c) {
            return Some(c.clone());
        }
    }
    None
}

fn which(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
