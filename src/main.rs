#![cfg_attr(all(windows, feature = "desktop"), windows_subsystem = "windows")]

#[cfg(feature = "desktop")]
mod desktop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|a| a == "--scan") {
        if args.len() != 3 {
            return Err("Usage: media-shelf --scan <movies|series> <folder>".into());
        }
        let category = match args[1].to_str() {
            Some("movies") => media_shelf::Category::Movies,
            Some("series") => media_shelf::Category::Series,
            _ => return Err("Category must be movies or series".into()),
        };
        let source = media_shelf::Source {
            category,
            path: args[2].clone().into(),
        };
        let report = media_shelf::index(
            &[source],
            &std::sync::atomic::AtomicBool::new(false),
            |_| {},
        );
        println!("{}", serde_json::to_string_pretty(&report)?);
        if !report.warnings.is_empty() {
            std::process::exit(2);
        }
        return Ok(());
    }
    if !args.is_empty() {
        return Err("Usage: media-shelf [--scan <movies|series> <folder>]".into());
    }
    #[cfg(feature = "desktop")]
    {
        desktop::run().map_err(|e| e.to_string())?;
    }
    #[cfg(not(feature = "desktop"))]
    {
        return Err("Headless build: use --scan <movies|series> <folder>".into());
    }
    #[allow(unreachable_code)]
    Ok(())
}
