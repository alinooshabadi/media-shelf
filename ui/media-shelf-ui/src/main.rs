//! Media Shelf — desktop application.
//!
//! The GUI is immediate-mode CUI (vendored, compiled by `build.rs`); this
//! binary owns the window via `cui_app_run` and renders from a Rust frame
//! callback over the `media-shelf` core.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use media_shelf::{Category, ScanReport, Source};

/// Scan one folder headlessly and print the report as JSON (exit 2 on warnings).
pub fn headless_scan(
    category: Category,
    folder: &std::path::Path,
) -> Result<ScanReport, Box<dyn std::error::Error>> {
    let source = Source {
        category,
        path: folder.to_path_buf(),
    };
    let report = media_shelf::index(&[source], &std::sync::atomic::AtomicBool::new(false), |_| {});
    Ok(report)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|a| a == "--scan") {
        if args.len() != 3 {
            return Err("Usage: media-shelf --scan <movies|series> <folder>".into());
        }
        let category = match args[1].to_str() {
            Some("movies") => Category::Movies,
            Some("series") => Category::Series,
            _ => return Err("Category must be movies or series".into()),
        };
        let report = headless_scan(category, std::path::Path::new(&args[2]))?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        if !report.warnings.is_empty() {
            std::process::exit(2);
        }
        return Ok(());
    }
    if !args.is_empty() {
        return Err("Usage: media-shelf [--scan <movies|series> <folder>]".into());
    }
    media_shelf_ui::app::run()
}
