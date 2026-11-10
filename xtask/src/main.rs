//! `cargo xtask bundle <plugin> --release`
//!
//! nih-plug's bundler creates the VST3 and CLAP bundles. On top of that, this copies the plugin's
//! standalone app (`<plugin>-standalone.exe`) next to them under its display name, so
//! `target/bundled/` holds everything that can be installed or run.

use std::path::Path;

fn main() -> nih_plug_xtask::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    nih_plug_xtask::main()?;
    if args.first().map(String::as_str) == Some("bundle") {
        let profile = if args.iter().any(|a| a == "--release") { "release" } else { "debug" };
        for package in args[1..].iter().filter(|a| !a.starts_with('-')) {
            copy_standalone(package, profile);
        }
    }
    Ok(())
}

/// Display name from `bundler.toml` (`[package]` followed by `name = "..."`), or the package name.
fn display_name(package: &str) -> String {
    let Ok(text) = std::fs::read_to_string("bundler.toml") else {
        return package.to_string();
    };
    let mut in_section = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_section = line == format!("[{package}]");
        } else if in_section && line.starts_with("name") {
            if let Some(value) = line.split('=').nth(1) {
                return value.trim().trim_matches('"').to_string();
            }
        }
    }
    package.to_string()
}

fn copy_standalone(package: &str, profile: &str) {
    let exe = if cfg!(windows) { ".exe" } else { "" };
    let source = Path::new("target").join(profile).join(format!("{package}-standalone{exe}"));
    if !source.exists() {
        return;
    }
    let target = Path::new("target").join("bundled").join(format!("{}{exe}", display_name(package)));
    match std::fs::copy(&source, &target) {
        Ok(_) => println!("Created a standalone app at '{}'", target.display()),
        Err(err) => eprintln!("Could not copy the standalone app to '{}': {err}", target.display()),
    }
}
