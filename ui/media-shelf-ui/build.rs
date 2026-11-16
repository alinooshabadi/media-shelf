//! Builds the vendored CUI (v1.4.0) C library and links platform frameworks.
//! The source list mirrors vendor/CUI/CMakeLists.txt so the vendor tree stays
//! buildable by either system.
use std::path::{Path, PathBuf};

fn main() {
    let cui = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../vendor/CUI");
    for dir in ["include", "src", "third_party"] {
        println!("cargo:rerun-if-changed={}", cui.join(dir).display());
    }
    println!("cargo:rerun-if-changed=build.rs");

    let core = [
        "cui_core.c", "cui_theme.c", "cui_draw.c", "cui_text.c", "cui_layout.c", "cui_widgets.c",
        "cui_controls.c", "cui_textedit.c", "cui_overlay.c", "cui_data.c", "cui_charts.c",
        "cui_extras.c", "cui_actions.c", "cui_pickers.c", "cui_disclosure.c", "cui_grid.c",
        "cui_treetable.c", "cui_code.c", "cui_markdown.c", "cui_editor.c", "cui_diff.c",
        "cui_tools.c", "cui_hex.c", "cui_social.c", "cui_media.c", "cui_planning.c",
        "cui_chrome.c", "cui_dock.c", "cui_viz.c", "cui_motion.c", "cui_ai.c", "cui_image.c",
        "cui_nodes.c", "cui_flame.c", "cui_curves.c", "cui_timeline.c", "cui_dials.c",
        "cui_month.c", "cui_strings.c", "cui_devtools.c", "cui_graph.c", "cui_devpanels.c",
        "cui_regex.c", "cui_devviews.c", "cui_instrument.c", "cui_tour.c", "cui_photo.c",
        "cui_files.c", "cui_terminal.c", "cui_emoji.c",
    ];
    let optional = ["cui_font_data.c", "cui_inflate.c", "render/cui_render_sw.c", "render/cui_render_gl.c"];

    let is_mac = cfg!(target_os = "macos");
    let is_win = cfg!(target_os = "windows");
    let is_linux = cfg!(target_os = "linux");

    // Core: all pure-C sources, one translation unit each.
    let mut build = cc::Build::new();
    build
        .std("c99")
        .opt_level(2)
        .include(&cui.join("include"))
        .include(&cui.join("src"))
        .include(&cui.join("third_party"));
    for name in core.iter().chain(optional.iter()) {
        build.file(cui.join("src").join(name));
    }
    if is_win {
        build.file(cui.join("src/platform/cui_app_win32.c"));
        build.define("GL_SILENCE_DEPRECATION", None);
    } else if is_linux {
        build.file(cui.join("src/platform/cui_app_x11.c"));
    }
    // The app layer is plain C on Windows and Linux (compiled above), and
    // Objective-C on macOS (compiled in its own pass below).
    build.file(cui.join("src/platform/cui_app.c"));
    build.file(cui.join("src/platform/cui_dialogs.c"));
    build.compile("cui_core");
    if is_win {
        for lib in ["user32", "gdi32", "opengl32", "shell32", "imm32", "dwmapi", "comdlg32", "ole32"] {
            println!("cargo:rustc-link-lib={lib}");
        }
    }

    if is_mac {
        // The Cocoa backend is Objective-C with ARC; it needs module builds,
        // whose cache is pinned to OUT_DIR so sandboxed build dirs keep working.
        let mut objc = cc::Build::new();
        let out_dir = std::env::var("OUT_DIR").unwrap();
        objc
            .std("c99")
            .opt_level(2)
            .include(&cui.join("include"))
            .include(&cui.join("src"))
            .include(&cui.join("third_party"))
            .file(cui.join("src/platform/cui_app_cocoa.m"))
            .flag("-fobjc-arc")
            .flag("-fmodules")
            .flag(&format!("-fmodules-cache-path={}", Path::new(&out_dir).join("module-cache").display()));
        objc.compile("cui_cocoa");
        for framework in ["Cocoa", "QuartzCore", "OpenGL"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
    if is_linux {
        for lib in ["X11", "dl", "m", "pthread"] {
            println!("cargo:rustc-link-lib={lib}");
        }
    }
}
