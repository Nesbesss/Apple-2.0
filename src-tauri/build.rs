use std::{env, fs, path::{Path, PathBuf}, process::Command};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        build_speech_helper();
    }
    tauri_build::build()
}

/// Compiles the Swift dictation sidecar into `binaries/`, where Tauri's `externalBin` expects it.
/// The helper's Info.plist is embedded so macOS can show the microphone and speech prompts for it.
fn build_speech_helper() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let arch = env::var("CARGO_CFG_TARGET_ARCH").expect("cargo sets CARGO_CFG_TARGET_ARCH");
    let swift_arch = match arch.as_str() {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        other => panic!("speech helper does not support {other}"),
    };

    let source = manifest.join("speech-helper/speech-helper.swift");
    let plist = manifest.join("speech-helper/Info.plist");
    let out_dir = manifest.join("binaries");
    let out = out_dir.join(format!("speech-helper-{arch}-apple-darwin"));
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-changed={}", plist.display());

    fs::create_dir_all(&out_dir).expect("create binaries dir");
    // Only rewrite the helper when its inputs changed. Tauri watches this file, so rewriting it on
    // every build would trigger another build, and `tauri dev` would loop forever.
    if is_up_to_date(&out, &[&source, &plist]) {
        return;
    }
    let status = Command::new("swiftc")
        .args(["-O", "-target", &format!("{swift_arch}-apple-macos26.0")])
        .arg(&source)
        .arg("-o")
        .arg(&out)
        .args(["-Xlinker", "-sectcreate", "-Xlinker", "__TEXT", "-Xlinker", "__info_plist", "-Xlinker"])
        .arg(&plist)
        .status()
        .expect("failed to run swiftc (install the Xcode command line tools)");
    assert!(status.success(), "swiftc failed to build the speech helper");
}

/// True when `output` exists and is at least as new as every input.
fn is_up_to_date(output: &Path, inputs: &[&Path]) -> bool {
    let modified = |path: &Path| fs::metadata(path).and_then(|m| m.modified()).ok();
    let Some(built_at) = modified(output) else { return false };
    inputs.iter().all(|input| modified(input).is_some_and(|changed| changed <= built_at))
}
