# CUI documentation site

A static site that runs every CUI component live in the browser: the library
compiled to WebAssembly and drawn with a WebGL2 port of its OpenGL renderer
(or its software renderer). It also renders the guide in `docs/` and the
example apps in `examples/`, so what you see is exactly what the library does.

The published copy is at https://yappologistic.github.io/CUI/, built and
deployed from `master` by `.github/workflows/pages.yml`.

## Run it

```sh
web/tools/serve.sh            # builds on first run, then serves http://localhost:8765
web/tools/build.sh            # rebuild after changing the library, a scene or an example
web/tools/build.sh scenes     # faster: only the component scenes
PORT=9000 web/tools/serve.sh  # another port
web/tools/build.sh app my_app.c   # your own app, at http://localhost:8765/#/app/my_app
```

Building needs clang with the `wasm32` target, `wasm-ld`, `llvm-ar` and
python3. No Emscripten and no downloads. The output (`web/build/`,
`web/site/wasm/`, `web/site/catalog.js` and links to `assets/fonts`,
`docs/images` and `examples/`) is generated and ignored by git. `web/site/`
can then be served by any static file server.

## What is on the site

- **Overview**: what CUI is, how to get it, a live component stage, a grid of
  components you can use with the mouse and keyboard, and the numbers CUI is
  held to. Its backdrop is a public domain photograph drawn as an ordered
  dither (`site/js/dither.js`, credit in `site/img/README.md`).
- **Guide**: the pages in `docs/` (getting started, layout, theming, icons,
  custom components, integration, rendering, design principles).
- **Components**: every documented component on its own page: the live
  component, its variants, the C that draws it, its options and keyboard
  support. `[` and `]` step through the pages, Ctrl+K or `/` searches.
- **Showcase** and **Apps**: larger compositions and the example apps, with
  width presets for their smaller layouts.

Settings (top right) switch the theme and accent, the renderer (WebGL2 or
software), slow motion down for inspecting animations, force reduced motion
and set the pixel ratio. Every code block has a wrap toggle, and the choice is
remembered for the whole site.

## How it works

- `shim/`: a small libc (allocator, strings, printf, strtod, time) so plain
  `clang --target=wasm32` builds the library without a sysroot. Math calls the
  browser's `Math`.
- `host/web_host.c`: the browser platform layer: contexts, input, clipboard,
  the software renderer and draw data for WebGL2.
- `host/web_app.c`: `cui_app_run` for the browser, so `examples/*.c`, and any
  app whose `main` calls `cui_app_run`, build unchanged (with
  `-Dmain=web_app_main`). `build.sh app` builds yours the same way and copies its
  source beside it; the page opens any `#/app/<name>` it finds a module for.
- `host/scenes_web.c`: serves the scene table of `tests/component_shots.c`
  (the scenes that render the documentation images) plus
  `host/extra_scenes.inc` for components without a documentation image.
- `site/js/gl.js`: the OpenGL renderer (`src/render/cui_render_gl.c`) ported to
  WebGL2 with the same shaders.
- `tools/gen_catalog.py`: turns `docs/` into the page catalog and extracts each
  scene's source with file and line numbers.

Each preview gets its own WebAssembly instance, so a reset is exact and scenes
do not share state. A scene first runs the same scripted frames as the
screenshot tool, so it opens in its documented state, then goes live.

## Checks

```sh
cd web && node tools/check_shots.mjs && python3 tools/compare_shots.py
```

renders every scene at 2x in light and dark with the WebAssembly build and
compares it with `docs/images/components/`. Comparable images are pixel
identical, except the week calendar, whose "now" line follows the clock.
