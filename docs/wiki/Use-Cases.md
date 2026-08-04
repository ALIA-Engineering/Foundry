# Use Cases

## 1. System Monitoring Dashboard

Display CPU, memory, and network stats in a native window. No browser overhead eating the resources you are trying to monitor.

```html
<div style="display: flex; gap: 16px; padding: 20px; background-color: #0d1117">
    <div style="background-color: #161b22; padding: 16px; border-radius: 8px; width: 200px">
        <div style="color: #8b949e; font-size: 12px">CPU</div>
        <div id="cpu" style="color: #58a6ff; font-size: 32px; font-weight: bold">42%</div>
    </div>
    <div style="background-color: #161b22; padding: 16px; border-radius: 8px; width: 200px">
        <div style="color: #8b949e; font-size: 12px">Memory</div>
        <div id="mem" style="color: #3fb950; font-size: 32px; font-weight: bold">8.2 GB</div>
    </div>
</div>
```

Binary: ~8.7MB. Memory: under 30MB. Starts fast.

## 2. Kiosk / Point-of-Sale UI

Restaurant ordering screen, airport check-in, retail POS. Runs fullscreen, no browser chrome, no URL bar, no close button visible.

## 3. Embedded Device Interface

IoT dashboards, industrial control panels, medical device displays. Foundry runs on anything with a Vulkan driver. No Chromium means no 200MB overhead on a device with 512MB RAM.

## 4. Game Overlay / HUD

Render UI on top of a game. Since Foundry renders via wgpu, the rendering pipeline can be integrated into an existing wgpu-based game engine.

## 5. Internal Developer Tools

Build custom tools for your team without shipping Electron. Log viewers, config editors, deployment dashboards. Write them in HTML/CSS like you already know, get a native binary.

## 6. Digital Signage

Menu boards, information displays, lobby screens. Static content with occasional updates. Foundry renders it at GPU speed with minimal memory.

## Common Thread

All share one property: they need a visual interface but not a full browser.
