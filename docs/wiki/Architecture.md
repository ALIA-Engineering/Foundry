# Architecture

## Module Map

```
src/
  lib.rs       Runtime library (embeddable, used by compiled binaries)
  main.rs      CLI entry: dev (live preview + hot reload) and build (compile to binary)
  scene.rs     Scene graph, resolved styles, layout rects, animation state, style lerp
  html.rs      Single-pass HTML parser via html5ever (scene + styles + scripts in one walk)
  css.rs       CSS parser, compound selectors, @keyframes, cascade, :hover, transitions
  layout.rs    taffy-backed flexbox layout with font-aware text measurement
  render.rs    wgpu GPU renderer with SDF shader, dynamic clear color
  text.rs      glyphon/cosmic-text GPU text rendering with system font discovery
  events.rs    Hit testing, hover style swapping, event bubbling, scroll handling
  js.rs        boa JS engine with DOM mutation bridge (setTextContent, setStyle, addClass)
```

## Data Flow

```
index.html ──> parse_html_full() ──> SceneGraph + Styles + Scripts + ExternalPaths
                                          │
                      External CSS/JS ──> resolve + inline
                                          │
                      @keyframes ──────> scene.keyframes[]
                      :hover rules ────> node.hover_style
                      animation prop ──> node.animation (AnimationState)
                                          │
                      taffy ───────────> layout rects (x, y, w, h)
                                          │
                      wgpu SDF shader ─> GPU draw calls (quads + text)
                                          │
                 boa JS ──> DomMutation[] ──> apply ──> dirty flags ──> re-layout
```

## Binary Compilation Flow

```
foundry build index.html -o app.exe
  │
  ├── Read HTML source
  ├── Escape for Rust string literal (\n, \r, \t, \\, \")
  ├── Generate temp Cargo project:
  │     Cargo.toml (depends on foundry_runtime)
  │     src/main.rs (calls foundry_runtime::run_embedded(html, title))
  ├── cargo build --release
  ├── Copy binary to output path
  └── Clean up temp project

Output: standalone .exe, ~8.7MB for examples/counter.html, no external dependencies
```

## Key Design Decisions

**Single-pass parsing.** HTML is parsed once. Styles, scripts, and scene graph are all extracted in one DOM walk. No redundant parsing.

**SelectorSegment model.** Each space-separated CSS token is a "segment" containing one or more parts (tag, class, id). All parts in a segment match the same element. Segments form the ancestor chain. This correctly handles `div.btn.active` (one segment, three parts) vs `.card h1` (two segments, ancestor chain).

**Scene graph, not DOM.** Flat `Vec<SceneNode>` with index-based NodeId references. No reference counting, no tree pointers. Mutation and iteration are cheap.

**One-time CSS resolution.** Cascade, specificity, and inheritance computed at load time. JS mutations go through a mutation queue with dirty flags for incremental re-layout.

**SDF rendering.** One WGSL shader handles all quad types: solid, bordered, rounded, transparent. Anti-aliased edges via signed distance field. No separate shaders per primitive.

**Style lerp for animations.** `ResolvedStyle::lerp()` interpolates colors, opacity, padding, margin, border-radius, font-size between any two styles. Used by both CSS transitions (hover) and @keyframes animations.

**Runtime library.** `foundry_runtime` is a library crate that compiled binaries link against. Contains the full render pipeline. The CLI (`foundry`) is a thin wrapper that adds dev mode and build commands.

## Dependencies

| Crate | Purpose |
|-------|---------|
| wgpu 29 | GPU abstraction (Vulkan + Metal) |
| boa_engine 0.20 | Embedded JS engine |
| html5ever 0.39 | HTML parsing (from Servo) |
| taffy 0.7 | Flexbox layout |
| glyphon 0.11 | GPU text rendering |
| winit 0.30 | Window creation + events |
| notify 7 | File watching (hot reload) |
| clap 4 | CLI argument parsing |
| bytemuck 1 | GPU vertex byte casting |

Release binaries: 8.96MB for the `foundry` CLI, 8.73MB for the app built from examples/counter.html (LTO, strip, opt-level=z, panic=abort). Re-measure with `python benchmarks/bench_binary_size.py`.
