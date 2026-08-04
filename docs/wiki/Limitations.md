# Limitations

Foundry is honest about what it cannot do.

## CSS

**Not supported:** grid, media queries, calc(), custom properties (var()), pseudo-elements (::before/::after), float, transform, box-shadow, text-decoration, text-transform, white-space, cursor, table layout, gradients, background-image.

**Partially supported:** `display: block` approximated as flex-column. `display: inline` approximated as flex. Percentage units work for width/height but not all properties.

## JS

- `fetch(url)` exists but is blocking and returns `{ ok, status, text }`; no Promise, async/await or XMLHttpRequest
- No import, require, export
- No setTimeout, setInterval
- `localStorage` exists (file-backed); no sessionStorage, no window object
- boa is 10-100x slower than V8 for computation
- DOM bridge uses method syntax: `el.setTextContent(x)` not `el.textContent = x`

## Rendering

- No image rendering (planned)
- No SVG, canvas, video, or audio
- System fonts only (no font embedding yet)
- Latin scripts reliable; complex scripts (Arabic, Devanagari) may render incorrectly
- Grayscale antialiasing only (no subpixel/ClearType)
- Text input is visual only (no cursor, selection, keyboard input)

## Platform

- No accessibility (screen readers cannot read GPU pixels)
- No clipboard, drag-and-drop, system tray, or menu bar
- Vulkan + Metal backends (no DX12 in v0.1)
- DPI scaling works but fractional scales may blur slightly

## Build

- `foundry build` requires Rust toolchain on the build machine
- Clean release build takes ~106 s on the benchmark machine (incremental rebuilds ~52 s)

## When NOT to Use Foundry

- Full web compatibility needed (use Electron)
- Accessibility required (use native UI or webview)
- Network APIs, WebSocket, real-time data (use Electron)
- Text input with IME (use native UI)
- DevTools debugging needed (use a browser)

## When Foundry Works

- Dashboards and monitoring displays
- Kiosk and point-of-sale UIs
- Embedded device interfaces
- Game overlay UIs
- Internal tools where binary size matters
- Demos and prototypes
