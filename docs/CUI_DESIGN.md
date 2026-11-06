# CUI visual implementation

Reviewed on 6 November 2026. The application remains on egui/eframe 0.33.3 (Cargo requirement 0.33). No runtime dependency, backend, persistence, scan, metadata, file-operation, navigation, or selection logic was changed.

## Source of truth

- [Official principles](https://github.com/yappologistic/CUI/blob/7bfb39ddaa5d0fd2dd9aacb0850d4767e2449d7f/docs/design.md)
- [Theme implementation](https://github.com/yappologistic/CUI/blob/7bfb39ddaa5d0fd2dd9aacb0850d4767e2449d7f/src/cui_theme.c)
- [Widget implementation and interaction colors](https://github.com/yappologistic/CUI/blob/7bfb39ddaa5d0fd2dd9aacb0850d4767e2449d7f/src/cui_widgets.c)
- [Music example](https://github.com/yappologistic/CUI/blob/7bfb39ddaa5d0fd2dd9aacb0850d4767e2449d7f/examples/music.c)
- [Official screenshot collection](https://github.com/yappologistic/CUI/tree/7bfb39ddaa5d0fd2dd9aacb0850d4767e2449d7f/docs/images)

The live docs URL was unavailable through the web reader. The repository, its design documentation, examples, font assets, and rendered reference screenshots were available and inspected. The blue accent is an existing CUI option, rather than a new MediaShelf palette.

## Audit and changes

The previous theme forced light mode, used a cool tinted page, platform-dependent fonts, 6-point widget corners, and unbordered controls. Status colors were repeated as literal RGB values in several screens. Small actions and candidate rows fell below CUI's control sizes. Metadata used an extra padded surface, and text hierarchy included sizes outside CUI's scale.

`src/desktop/theme.rs` now owns the palette, typography, geometry, spacing, and field/checkbox/loading styles. All UI modules use these semantic colors. Font files are copied from CUI's assets and include their upstream licenses.

| Area | Applied values |
| --- | --- |
| Neutral colors | Exact upstream OKLab lightness values, converted to sRGB |
| Dark page / surface / raised | L 0.18 / 0.21 / 0.24 |
| Light page / surface / raised | L 1.0 / 1.0 / 1.0 |
| Border / strong border | L 0.915 / 0.665 light; 0.29 / 0.515 dark |
| Accent | CUI blue: #0562ef light, #6ba3ff dark |
| Status | Upstream green success, amber warning, red danger; paired with existing text |
| Fonts | Inter regular and medium; Geist medium for the large media title |
| Type | 12 small, 13 body/buttons, 17 section, 22 heading, 28 media title |
| Corners | 10 controls, 14 menus, 18 windows; checkbox corners scoped separately |
| Controls | 30 small, 36 default, 44 library rows; 14 horizontal button padding, 12 field padding |
| Spacing | 8 control gaps, 16 group margins, 24 regions, 32 section separation |
| Disabled | 45% opacity |
| Floating surfaces | Border plus close shadow; no new resting shadows |

Coverage includes the toolbar and menus, both libraries, series tree, season table, all five detail tabs, matching dialog, source editor, settings, file-operation progress/preview/report windows, warnings, and empty states. Existing native file/folder dialogs remain OS-rendered.

Settings now offers Light / Dark / System and Reduce motion. These are session preferences in egui's existing context; no persistence schema or application-state structure was added. Reduced motion disables egui transitions and replaces the app's animated spinners with a static status ring while retaining loading labels.

Folder links and sort headers use standard egui buttons, preserving their actions while providing keyboard activation. The previous glyph-only scrape menu is labeled More. Previous/Next pagination has readable names. Matching body content scrolls separately from its action footer; source/settings windows scroll when needed. Long preview text is constrained to its column.

## Validation

- `cargo build --locked`: passed.
- `cargo fmt --all --check`: passed.
- `cargo test --locked`: 80 passed.
- `cargo test --locked --no-default-features`: 70 passed.
- `cargo clippy --locked --all-targets`: completed with existing style warnings, including collapsible conditionals, map entry, test-module ordering, and type complexity. No new theme warning was reported.
- An additional `-D warnings` run failed on pre-existing warnings in backend files; those were left outside this visual change.

Existing render tests were expanded to cover both themes, both categories, 1080/1380-point widths, all detail tabs, and long matching summaries at 880/600-point heights. Their footer visibility check caught a checkbox-wrapper layout regression during development; it was fixed before the final test run.

A native macOS fixture build exercised the actual modified UI renderers with synthetic movie/show metadata and no library database. Screenshots cover 11 views in both themes: movie overview, cast, media files, artwork, trailer, series overview, season table, settings, sources, matching, and cleanup preview. Additional captures show menus, keyboard focus, reduced motion, and credits reached by scrolling.

Captures are 2760 × 1592 pixels, representing a 1380 × 796-point window including native chrome at 2× display scale. CUA supplied compressed captures; deliverables were converted to actual PNG and normalized to sRGB. A reference comparison uses unscaled 2× crops from official light/dark input screenshots beside the application. Reference and application field widths/content differ; this is a component comparison, not a full-screen pixel-difference test. Sampled dark page/surface RGB values are (18,18,18) and (24,24,24), matching the source tokens. The light page is (255,255,255).

The default production application was launched, but loading its saved library stalled before a usable window could be inspected. The fixture build bypassed that loading step for native visual review. Existing automated tests cover persistence, file operations, NFO handling, indexing, and scrape flows; live NAS scanning, remote TMDB requests, credentials changes, and real file moves were not performed during visual review. Full live-workflow verification remains outstanding. A subsequent release-build launch completed and displayed the real saved library and cached poster; the README includes that populated light-theme capture alongside labeled dark-theme fixtures.

## Known differences from CUI

This is a CUI-based egui implementation, not verified pixel-perfect equivalence.

- Egui retains its interaction model. Keyboard activation is available, but focus visuals can also appear after clicks; CUI's keyboard-only outer focus ring is not reproduced globally. Native egui accessibility-tree coverage is unchanged.
- The 140 ms style timing is installed, but egui does not reproduce CUI's independent spring curves, 0.97/0.98 press scales, 280/340 ms floating entrances, or 160 ms exits. Hover/press outlines and fills retain egui behavior. Solid primary buttons do not reproduce CUI's upper-rim lighting or its exact interpolated hover colors.
- Egui exposes a single floating shadow; the main CUI shadow is approximated, without the separate contact-shadow layer or negative spread.
- Table row selections remain rectangular, and some tree rows retain egui's compact selection geometry. Menu item radii and every individual small control are not a custom reimplementation of CUI widgets.
- Inter/Geist are the upstream fonts, but text layout, rasterization, line-height, and truncation remain egui's. OpenType tabular-figure shaping is not added. Aligned values keep their existing fixed table columns and formatting.
- The app's existing 1080 × 720 desktop minimum and pane/navigation structure are preserved. CUI's 360-point phone layout and drawer adaptation are not introduced. Existing title/path truncation keeps its original tooltips.
- Appearance/reduced-motion preferences reset on restart. The OS reduced-motion preference is not automatically detected; use the explicit setting. Image-loader animations remain under egui_extras rather than the app's spinner helper.
- The original content and workflows are retained, including some legacy labels, the TMDB brand logo, and OS-controlled dialogs. Reference images use different content and layouts; there is no claim of full-screen identity.
- Native screenshots use fixtures; cast/artwork/trailer empty states were captured, rather than populated remote-media cases. Automated tests also exercise local episode artwork.
