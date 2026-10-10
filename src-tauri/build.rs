use std::{env, fs, path::{Path, PathBuf}, process::Command};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        build_speech_helper();
    }
    tauri_build::build()
}

/// Builds the Swift dictation sidecar with SwiftPM and copies it into `binaries/`, where Tauri's
/// `externalBin` expects it. The package pins FluidAudio, so the first build fetches it.
fn build_speech_helper() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let arch = env::var("CARGO_CFG_TARGET_ARCH").expect("cargo sets CARGO_CFG_TARGET_ARCH");
    let swift_arch = match arch.as_str() {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        other => panic!("speech helper does not support {other}"),
    };

    let package = manifest.join("speech-helper");
    let out_dir = manifest.join("binaries");
    let out = out_dir.join(format!("speech-helper-{arch}-apple-darwin"));
    let inputs = helper_inputs(&package);
    for input in &inputs {
        println!("cargo:rerun-if-changed={}", input.display());
    }

    fs::create_dir_all(&out_dir).expect("create binaries dir");
    // Only rewrite the helper when its inputs changed. Tauri watches this file, so rewriting it on
    // every build would trigger another build, and `tauri dev` would loop forever.
    let input_refs: Vec<&Path> = inputs.iter().map(PathBuf::as_path).collect();
    if is_up_to_date(&out, &input_refs) {
        return;
    }

    // Build output goes to OUT_DIR, inside the target directory, so it never lands in the watched source tree.
    let scratch = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("speech-helper-build");
    let build = Command::new("swift")
        .args(["build", "-c", "release", "--arch", swift_arch, "--package-path"])
        .arg(&package)
        .arg("--scratch-path")
        .arg(&scratch)
        .status()
        .expect("failed to run swift build (install the Xcode command line tools)");
    assert!(build.success(), "swift build failed for the speech helper");

    let bin_dir = Command::new("swift")
        .args(["build", "-c", "release", "--arch", swift_arch, "--package-path"])
        .arg(&package)
        .arg("--scratch-path")
        .arg(&scratch)
        .arg("--show-bin-path")
        .output()
        .expect("failed to ask SwiftPM where the helper is");
    let bin_dir = PathBuf::from(String::from_utf8_lossy(&bin_dir.stdout).trim());
    fs::copy(bin_dir.join("speech-helper"), &out).expect("copy the speech helper into binaries/");
}

/// Everything the helper is built from: the package, its lock file, the plist, and every source file.
fn helper_inputs(package: &Path) -> Vec<PathBuf> {
    let mut inputs = vec![package.join("Package.swift"), package.join("Package.resolved"), package.join("Info.plist")];
    collect_files(&package.join("Sources"), &mut inputs);
    inputs
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// True when `output` exists and is at least as new as every input.
fn is_up_to_date(output: &Path, inputs: &[&Path]) -> bool {
    let modified = |path: &Path| fs::metadata(path).and_then(|m| m.modified()).ok();
    let Some(built_at) = modified(output) else { return false };
    inputs.iter().all(|input| modified(input).is_some_and(|changed| changed <= built_at))
}
