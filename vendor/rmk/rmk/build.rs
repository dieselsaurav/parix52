#[path = "./build_common.rs"]
mod common;

use std::path::Path;
use std::process::Command;
use std::{env, fs};

fn main() {
    // Set the compilation target configuration
    let mut cfgs = common::CfgSet::new();
    common::set_target_cfgs(&mut cfgs);

    println!("cargo:rerun-if-changed=build.rs");
    // The hash guards the flash store: a store written by a different build
    // is wiped and re-initialised from the compiled keymap. It therefore has
    // to change whenever the keymap does. The keymap lives in keyboard.toml,
    // which this crate never reads, so cargo happily reused a cached hash
    // across keymap edits; a board then loaded the previous keymap out of
    // its store and the new bindings never appeared. Hash the toml itself.
    println!("cargo:rerun-if-env-changed=KEYBOARD_TOML_PATH");
    if let Ok(toml) = env::var("KEYBOARD_TOML_PATH") {
        println!("cargo:rerun-if-changed={toml}");
    }

    // Compute build hash and write to constants.rs
    let build_hash = compute_build_hash();
    let constants = format!(
        "#[allow(clippy::redundant_static_lifetimes)]\npub(crate) const BUILD_HASH: u32 = {build_hash:#010x};\n"
    );

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("constants.rs");
    fs::write(&dest_path, constants).expect("Failed to write constants.rs file");
}
fn compute_build_hash() -> u32 {
    // Get the short hash of the latest Git commit. Use "unknown" if it fails
    let commit_id = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    // The keyboard config, when the build knows where it is. Its bytes go
    // into the hash so any keymap change invalidates the store. No time
    // component: two builds of the same commit and config produce the same
    // hash, so a reflash of an unchanged firmware keeps the user's Vial
    // edits instead of wiping them.
    let toml = env::var("KEYBOARD_TOML_PATH")
        .ok()
        .and_then(|p| fs::read(p).ok())
        .unwrap_or_default();

    // Combine data and compute CRC32
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(commit_id.as_bytes());
    hasher.update(b"_");
    hasher.update(&toml);
    hasher.finalize()
}
