<p align="center">
  <img src="assets/logo.png" alt="Foundry" width="280">
  <br><br>
  <b>Cast web into native metal.</b>
  <p align="center">
    <a href="https://crates.io/crates/alia-foundry"><img src="https://img.shields.io/crates/v/alia-foundry?color=green" alt="crates.io"></a>
    <a href="https://github.com/TxsharDev/Foundry/blob/master/LICENSE"><img src="https://img.shields.io/badge/license-Apache%202.0-green" alt="License"></a>
    <a href="#"><img src="https://img.shields.io/badge/tests-137%20pass-green" alt="Tests"></a>
    <a href="#"><img src="https://img.shields.io/badge/app%20binary-8.7MB-blue" alt="Binary Size"></a>
  </p>
  <p align="center">
    <a href="docs/wiki/Quick-Start.md">Quick Start</a> &nbsp;|&nbsp;
    <a href="docs/wiki/How-It-Works.md">How It Works</a> &nbsp;|&nbsp;
    <a href="docs/wiki/CSS-Reference.md">CSS Reference</a> &nbsp;|&nbsp;
    <a href="docs/wiki/Limitations.md">Limitations</a> &nbsp;|&nbsp;
    <a href="docs/wiki/Architecture.md">Architecture</a>
  </p>
</p>

---

Electron ships a 200MB Chromium to run a chat app. Your "native" app is a web browser pretending to be software.

Foundry renders HTML, CSS, and JavaScript with its own GPU renderer instead of a browser engine. No Chromium, no webview.

`foundry build` is a code generator plus a `cargo build` wrapper, not a compiler of its own: it inlines your CSS and JS into the document, embeds that document as a string literal in a generated `main.rs`, and shells out to `cargo build --release` to link it against the Foundry runtime. The result is a single standalone binary -- 8.7 MB for the counter example -- that parses, cascades, lays out, runs its JS and rasterizes entirely on its own. Building needs a Rust toolchain; running the output does not.

```bash
foundry build index.html -o app.exe   # compile to standalone native binary
foundry dev index.html                # live preview + hot reload
```

Write HTML. Get a native app. That's it.

<p align="center">
  <img src="assets/showcase.png" alt="Foundry Showcase" width="700">
  <br>
  <em>Landing page with animations -- compiled from HTML to native GPU binary</em>
</p>

<p align="center">
  <img src="assets/dashboard.png" alt="Foundry Dashboard" width="700">
  <br>
  <em>117-node monitoring dashboard -- GPU-rendered, no browser</em>
</p>

## What Ships

- **Binary output** -- `foundry build` generates a Rust project around your document and has cargo link it into a standalone .exe
- **GPU rendering** -- every pixel drawn by wgpu (Vulkan/Metal) via SDF shaders
- **CSS animations** -- `@keyframes`, `animation`, `transition`, `:hover` pseudo-class
- **Compound selectors** -- `.card h1`, `div.btn`, `.card.active` all work correctly
- **Hot reload** -- edit your files, the window updates live
- **External files** -- `<link>` stylesheets and `<script src>` resolved at compile time
- **Embedded JS** -- event handlers, DOM mutations, `localStorage` and a blocking `fetch()` via boa (pure-Rust JS engine)
- **Flexbox layout** -- powered by taffy (from Dioxus/Bevy)
- **137 tests** (plus one gated behind network access), ~4,600 lines of implementation Rust, ~2,400 lines of test Rust, 4 demo apps

## Quick Start

```bash
cargo install alia-foundry
```

Create `hello.html`:

```html
<!DOCTYPE html>
<html>
<head>
    <style>
        @keyframes fadeIn {
            from { opacity: 0 }
            to { opacity: 1 }
        }
        body {
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            background-color: #1a1a2e;
        }
        .card {
            background-color: #16213e;
            padding: 40px;
            border-radius: 12px;
            display: flex;
            flex-direction: column;
            align-items: center;
            gap: 20px;
            animation: fadeIn 1s;
        }
        h1 { color: #e94560; }
        .btn {
            background-color: #e94560;
            color: white;
            padding: 12px 32px;
            border-radius: 8px;
            transition: all 0.2s;
        }
        .btn:hover {
            background-color: #c73650;
        }
    </style>
</head>
<body>
    <div class="card">
        <h1>Counter</h1>
        <div id="count" style="font-size: 64px; color: white">0</div>
        <div class="btn" onclick="increment()">+</div>
    </div>
    <script>
        var count = 0;
        function increment() {
            count = count + 1;
            document.getElementById("count").setTextContent(String(count));
        }
    </script>
</body>
</html>
```

Run it:
```bash
foundry dev hello.html        # live preview with hot reload
foundry build hello.html -o hello.exe  # compile to native binary
./hello.exe                   # runs standalone, no Foundry needed
```

## Foundry vs. Electron

| | Foundry | Electron |
|---|---|---|
| Binary size | **8.7 MB** (counter example) | 200+ MB |
| Runtime | None (standalone) | Chromium |
| Rendering | GPU (wgpu/Vulkan) | Chromium/Skia |
| JS engine | boa (embedded) | V8 |
| CSS animations | Yes (@keyframes, transitions) | Yes (full) |
| Web API coverage | Subset (`localStorage`, blocking `fetch`, no Promises) | Full |
| Startup | Fast (GPU init + first frame) | 1-3 seconds |

Honest caveat: Electron runs full web applications. Foundry runs a useful subset. The comparison is binary size and rendering architecture, not feature parity.

## How It Works

1. **Parse** HTML into a scene graph via html5ever (single pass: styles, scripts, elements extracted together)
2. **Resolve** CSS cascade, specificity, inheritance, `:hover`, and `@keyframes` at load time
3. **Layout** via taffy (flexbox) with parent font-size inheritance for text nodes
4. **Render** with wgpu via SDF fragment shader (rounded corners, borders, anti-aliasing in one draw call)
5. **Text** via glyphon/cosmic-text with system font discovery
6. **Animate** transitions and keyframe animations per-frame with style interpolation
7. **Execute** JS event handlers via boa with DOM mutation bridge
8. **Build** inlines `<link>`/`<script src>` files, escapes the document into a string literal inside a generated `src/main.rs`, writes a `Cargo.toml` that depends on the Foundry runtime, and invokes `cargo build --release`

Steps 1-7 are Foundry's own code. Step 8 is codegen plus a call to cargo -- Foundry emits Rust source, not machine code.

## Supported CSS

**Layout:** display (flex/block/inline/none), flex-direction, justify-content, align-items, flex-wrap, gap, flex-grow, flex-shrink

**Box model:** margin, padding, width, height, min/max-width/height, border, border-radius

**Visual:** background-color, color, opacity, border-color, z-index

**Text:** font-size, font-weight, font-family, text-align, line-height

**Position:** relative, absolute, fixed, top/left/right/bottom

**Units:** px, %, em, rem, vh, vw

**Pseudo-classes:** :hover (with style swapping and relayout)

**Animations:** @keyframes, animation (name, duration, delay, iteration-count, direction), transition (all properties, duration)

**Overflow:** visible, hidden, scroll

**Selectors:** tag, .class, #id, *, compound (div.btn, .card.active), descendant (.card h1, #main .text)

## Supported JS

```javascript
var el = document.getElementById("myId");
el.setTextContent("new text");
el.setStyle("background-color", "#ff0000");
el.addClass("active");
el.removeClass("hidden");
console.log("debug output");

localStorage.setItem("key", "value");   // persisted to a JSON file on disk
var saved = localStorage.getItem("key");
localStorage.removeItem("key");
localStorage.clear();

var res = fetch("https://example.com");  // blocking, no Promise
if (res.ok) { console.log(res.status, res.text); }
```

Events: `onclick`, `onmouseenter`, `onmouseleave`

## Architecture

```
src/
  lib.rs       Runtime library (used by compiled binaries)
  main.rs      CLI: dev (live preview) and build (codegen + cargo build)
  codegen.rs   Generates the Cargo.toml and main.rs of the built project
  scene.rs     Scene graph, styles, layout rects, animation state, lerp
  html.rs      Single-pass HTML parser (html5ever -> scene + styles + scripts)
  css.rs       CSS parser, compound selectors, @keyframes, cascade resolver
  layout.rs    taffy-backed flexbox layout with font-aware text measurement
  render.rs    wgpu GPU renderer: SDF quads, dynamic clear color
  text.rs      glyphon GPU text rendering with system fonts
  events.rs    Hit testing, hover styles, event bubbling, scroll
  js.rs        boa JS engine with DOM mutation bridge
```

## Examples

| Example | Nodes | Features |
|---------|-------|----------|
| `counter.html` | 14 | Click events, hover effects, JS state |
| `todo.html` | 28 | Task list, status tracking |
| `dashboard.html` | 117 | Stats cards, service table, action buttons, hover transitions |
| `showcase.html` | 78 | Animated landing page, @keyframes fadeIn/pulse, feature cards |

## What This Does NOT Do

- **Full CSS.** No grid, no media queries, no calc(), no custom properties, no pseudo-elements.
- **Accessibility.** GPU rendering produces pixels, not accessible UI trees.
- **Full web platform.** `localStorage` and a blocking `fetch()` shipped in v0.2; Promises, async/await, timers, WebSocket and the rest of the platform did not.
- **Fast JS.** boa is 10-100x slower than V8 for computation.
- **Image rendering.** Not yet (planned).

Foundry is for: dashboards, kiosk UIs, embedded displays, internal tools, game overlays -- apps where you want web-like development without the 200MB browser tax.

## Roadmap

**v0.1** -- Full pipeline: HTML/CSS/JS to GPU window and standalone binary. SDF rendering, flexbox, animations, transitions, :hover, compound selectors, hot reload, embedded JS.

**v0.2.1** (current) -- `localStorage` backed by a JSON file, blocking `fetch()` over ureq. 137 tests. 8.7 MB app binary.

**v0.3** -- Image rendering (GPU textures), CSS grid, font embedding, text input with cursor.

**v0.4** -- Basic accessibility (UI Automation on Windows), multi-window, clipboard integration.

## Prior Art

| Tool | What It Does | How Foundry Relates |
|------|-------------|-------------------|
| **Blitz** | HTML/CSS rendered with Servo's Stylo plus Vello, no browser engine | The same thesis as Foundry and further along: real Stylo cascade, block/inline/table/grid layout, Vello vector rendering. Foundry's CSS engine is hand-written and covers much less. |
| **Dioxus (native)** | Rust UI framework whose native renderer is built on Blitz | Shares the renderer above; RSX components rather than HTML files. Foundry takes plain `.html` as its input format. |
| **Slint** | Own declarative language, GPU or software renderer, embedded focus | Not web technologies -- `.slint` markup instead of HTML/CSS. Comparable rendering approach and binary sizes. |
| **Makepad** | Rust UI framework with a shader-based renderer and a live-reloading DSL | Also draws widgets with SDF-style shaders, but through its own DSL rather than HTML/CSS. |
| **Electron** | Ships Chromium as the renderer | Foundry has no browser engine and renders directly on the GPU |
| **Tauri** | Uses the system webview | Still a browser engine (WebKit/WebView2). Foundry is GPU-native. |
| **Flutter** | Custom renderer, Dart language | Not web technologies. Foundry uses HTML/CSS/JS. |
| **Sciter** | Lightweight embeddable HTML/CSS engine | GPU-accelerated as well (Skia, Direct2D and OpenGL backends) and far more complete. Closed source, C++. Foundry is Apache-2.0 Rust. |
| **Servo** | Research browser engine in Rust | Full browser. Foundry uses Servo's parsing crate (html5ever) with a custom GPU renderer. |

Blitz is the closest comparison and the more mature project. Foundry's distinguishing bet is the single-file `.html` in, single-file native binary out workflow, not renderer completeness.

## License

Apache-2.0 | ALIA Labs

Built by [Tushar Sharma](https://github.com/TxsharDev) at ALIA Labs.
