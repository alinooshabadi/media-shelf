# Desktop UI design

Media Shelf uses egui/eframe 0.33.3. The desktop theme lives in `src/desktop/theme.rs`; the existing backend, persistence, metadata, scanning, file-operation and selection workflows are unchanged.

## Shared styling

Use semantic palette values rather than literal colors in individual screens. Most surfaces are neutral. Blue identifies selections, links, focus and primary actions. Green, amber and red identify success, attention and failure alongside text.

| Area | Values |
| --- | --- |
| Dark page / surface / raised | OKLab L 0.18 / 0.21 / 0.24 |
| Light page / surface / raised | OKLab L 1.0 / 1.0 / 1.0 |
| Border / strong border | L 0.915 / 0.665 light; 0.29 / 0.515 dark |
| Accent | #0562ef light; #6ba3ff dark |
| Fonts | Inter regular and medium; Geist medium for large media titles |
| Type sizes | 12 small, 13 body/buttons, 17 section, 22 heading, 28 media title |
| Corner radii | 10 controls, 14 menus, 18 windows |
| Control heights | 30 small, 36 default, 44 library rows |
| Padding | 14 horizontal button padding; 12 field padding |
| Spacing | 8 control gaps, 16 group margins, 24 regions, 32 sections |
| Disabled opacity | 45% |
| Floating surfaces | Hairline border and close shadow |

Bundled fonts retain their licenses in `assets/fonts/`.

The shared theme covers the toolbar, menus, movie and series libraries, season table, detail tabs, matching dialog, sources, settings, file tools, warnings and empty states. Native file/folder dialogs retain their operating-system appearance.

## Appearance and interaction

Settings offers Light, Dark, System and Reduce motion. These preferences last for the session. Reduced motion disables egui transitions and replaces application spinners with static status rings while retaining loading labels.

Folder links and sort headers use standard keyboard-activatable egui buttons. Matching content scrolls separately from its action footer. Source and settings windows scroll when space is limited. Long preview text wraps within its column.

## Verification

- Build, release packaging and formatting passed.
- Desktop tests: 80 passed. Headless tests: 70 passed.
- Clippy completed with existing repository warnings.
- Render tests cover both themes, both media categories, 1080/1380-point widths, every detail tab, and long matching summaries at 880/600-point heights.
- Native macOS review covered 11 views in both themes using isolated fixtures, plus menu, focus, reduced-motion and credits states.
- The release build also displayed the real saved library and cached poster. The README identifies which screenshots use sample data.

## Current limits

Egui supplies focus visuals, hover feedback, text rasterization, table selection geometry and floating shadows. Aligned values retain their existing table columns and formatting; additional OpenType tabular shaping is not installed.

The desktop minimum remains 1080 × 720. Appearance preferences reset on restart. Reduced motion is selected explicitly rather than detected from the OS, and image-loader animations remain managed by egui_extras.

Live TMDB requests, NAS rescans, credential changes and real file moves were not exercised during visual review. Automated tests cover the underlying persistence, indexing, metadata and file-operation behavior.
