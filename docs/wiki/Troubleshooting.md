# Troubleshooting

## "No suitable GPU adapter found"

Foundry requires a Vulkan-capable GPU. Check:

1. Install the latest GPU drivers
2. Verify Vulkan support: `vulkaninfo` (install Vulkan SDK if needed)
3. On Windows, ensure the Vulkan runtime is installed (usually bundled with GPU drivers)

## "Failed to create window"

The display server must be running. On Linux, Foundry requires X11 or Wayland. On headless servers, use a virtual framebuffer.

## "JS error: ..."

Common causes:
- Using browser APIs that Foundry does not support (`Promise`, `setTimeout`, async/await)
- Using property syntax instead of method syntax: use `el.setTextContent(x)` not `el.textContent = x`
- Referencing an element ID that does not exist in the HTML

## "foundry: loaded 0 nodes"

Your HTML might be malformed, or the `<body>` tag is missing content. Foundry only renders elements inside `<body>`.

## Blank White Window

The window opens but nothing renders:
- Check that your elements have explicit dimensions or content (empty divs with no width/height are invisible)
- Verify that `background-color` is set (default is transparent)
- Check the console output for JS errors

## Text Not Showing

- Text must be inside a visible element
- Font-size must be greater than 0
- Color must not be transparent or match the background
- The element must have non-zero layout dimensions

## Performance Issues

- Foundry renders via GPU. If you see low frame rates, check GPU utilization
- Large numbers of DOM nodes (1000+) may slow down the layout pass
- Complex JS in event handlers runs through boa, which is slower than V8

## Building From Source

```bash
git clone https://github.com/TxsharDev/foundry
cd foundry
cargo build --release
```

Requires:
- Rust 1.87+ (wgpu 29's minimum)
- Vulkan SDK (for GPU rendering)
- C/C++ toolchain (for native dependencies)
