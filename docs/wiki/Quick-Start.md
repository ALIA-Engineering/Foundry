# Quick Start

## Install

```bash
cargo install alia-foundry
```

## Your First App

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
            background-color: #0d1117;
            color: #58a6ff;
        }
        h1 {
            font-size: 48px;
            animation: fadeIn 1s;
        }
    </style>
</head>
<body>
    <h1>Hello from Foundry</h1>
</body>
</html>
```

### Dev mode (live preview + hot reload):
```bash
foundry dev hello.html
```
A native GPU-rendered window opens. Edit the file, the window updates automatically.

### Build mode (compile to standalone binary):
```bash
foundry build hello.html -o hello.exe
./hello.exe    # runs standalone, no Foundry needed
```
Output: an 8.7MB native binary (measured on examples/counter.html).

## Adding Interactivity

```html
<style>
    .btn {
        background-color: #238636;
        color: white;
        padding: 12px 24px;
        border-radius: 6px;
        transition: all 0.2s;
    }
    .btn:hover {
        background-color: #2ea043;
    }
</style>

<div id="greeting" style="font-size: 32px; color: white">Click the button</div>
<div class="btn" onclick="greet()">Say Hello</div>

<script>
    function greet() {
        document.getElementById("greeting").setTextContent("Hello, World!");
        document.getElementById("greeting").setStyle("color", "#58a6ff");
    }
</script>
```

## Adding Animations

```html
<style>
    @keyframes pulse {
        0% { opacity: 1 }
        50% { opacity: 0.5 }
        100% { opacity: 1 }
    }
    .status {
        animation: pulse 2s infinite;
    }
</style>
<div class="status">System Online</div>
```

## Supported Elements

`div`, `span`, `p`, `h1`-`h6`, `button`, `input`, `img`, `a`, `ul`, `ol`, `li`

## Supported Events

`onclick`, `onmouseenter`, `onmouseleave`

## External Files

```html
<link rel="stylesheet" href="styles.css">
<script src="app.js"></script>
```

Both resolved at compile time relative to the HTML file.

## Next Steps

- [How It Works](How-It-Works.md) -- the compile-time pipeline
- [CSS Reference](CSS-Reference.md) -- all supported properties
- [Limitations](Limitations.md) -- what Foundry cannot do (yet)
- [Use Cases](Use-Cases.md) -- where Foundry shines
