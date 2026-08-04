#!/usr/bin/env python3
"""Measure Foundry binary size and compare against Electron/Tauri.

Two numbers matter and they are not the same:

* the `foundry` CLI itself (compiler + dev server + runtime), and
* the app binary that `foundry build` produces, which is what a user ships.

The second one is measured by actually building `examples/counter.html`.
Pass --no-app to skip that (it takes a few minutes).
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile

FOUNDRY_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BINARY_PATH = os.path.join(FOUNDRY_ROOT, "target", "release", "foundry.exe")
if not os.path.exists(BINARY_PATH):
    # Unix fallback
    BINARY_PATH = os.path.join(FOUNDRY_ROOT, "target", "release", "foundry")

# Electron and Tauri reference sizes (well-documented industry numbers)
# Electron: minimal hello-world app built with electron-builder
# Tauri: minimal hello-world app built with tauri build
ELECTRON_MIN_MB = 150.0   # electron-builder minimal Windows .exe
ELECTRON_TYPICAL_MB = 200.0
TAURI_MIN_MB = 2.5         # tauri v2 minimal Windows .exe
TAURI_TYPICAL_MB = 8.0


EXAMPLE_HTML = os.path.join(FOUNDRY_ROOT, "examples", "counter.html")


def measure_built_app():
    """Build an example with the CLI and return the produced binary size."""
    out_dir = tempfile.mkdtemp(prefix="foundry_bench_")
    out = os.path.join(out_dir, "counter.exe" if os.name == "nt" else "counter")
    try:
        proc = subprocess.run(
            [BINARY_PATH, "build", EXAMPLE_HTML, "-o", out],
            capture_output=True,
            text=True,
        )
        if proc.returncode != 0 or not os.path.exists(out):
            print("WARNING: could not build the example app:")
            print(proc.stdout[-2000:])
            print(proc.stderr[-2000:])
            return None
        return os.path.getsize(out)
    finally:
        shutil.rmtree(out_dir, ignore_errors=True)


def main():
    if not os.path.exists(BINARY_PATH):
        print(f"ERROR: binary not found at {BINARY_PATH}")
        print("Run: cargo build --release")
        sys.exit(1)

    size_bytes = os.path.getsize(BINARY_PATH)
    size_mb = size_bytes / (1024 * 1024)

    app_bytes = None
    if "--no-app" not in sys.argv:
        print("Building examples/counter.html to measure the shipped app size...")
        app_bytes = measure_built_app()

    results = {
        "foundry_binary_bytes": size_bytes,
        "foundry_binary_mb": round(size_mb, 2),
        "electron_typical_mb": ELECTRON_TYPICAL_MB,
        "electron_min_mb": ELECTRON_MIN_MB,
        "tauri_typical_mb": TAURI_TYPICAL_MB,
        "tauri_min_mb": TAURI_MIN_MB,
        "ratio_vs_electron": round(ELECTRON_TYPICAL_MB / size_mb, 1),
        "ratio_vs_tauri_typical": round(TAURI_TYPICAL_MB / size_mb, 1),
    }

    if app_bytes is not None:
        app_mb = app_bytes / (1024 * 1024)
        results["built_app_bytes"] = app_bytes
        results["built_app_mb"] = round(app_mb, 2)
        results["built_app_source"] = "examples/counter.html"
        results["ratio_app_vs_electron"] = round(ELECTRON_TYPICAL_MB / app_mb, 1)

    print("=" * 60)
    print("BINARY SIZE COMPARISON")
    print("=" * 60)
    print(f"{'Framework':<20} {'Size':>10} {'vs Foundry':>15}")
    print("-" * 60)
    print(f"{'Foundry CLI':<20} {size_mb:>8.2f} MB {'(baseline)':>15}")
    if app_bytes is not None:
        app_mb = app_bytes / (1024 * 1024)
        print(f"{'Foundry app':<20} {app_mb:>8.2f} MB {'(shipped)':>15}")
    print(f"{'Tauri (min)':<20} {TAURI_MIN_MB:>8.2f} MB {TAURI_MIN_MB/size_mb:>14.1f}x")
    print(f"{'Tauri (typical)':<20} {TAURI_TYPICAL_MB:>8.2f} MB {TAURI_TYPICAL_MB/size_mb:>14.1f}x")
    print(f"{'Electron (min)':<20} {ELECTRON_MIN_MB:>8.2f} MB {ELECTRON_MIN_MB/size_mb:>14.1f}x")
    print(f"{'Electron (typical)':<20} {ELECTRON_TYPICAL_MB:>8.2f} MB {ELECTRON_TYPICAL_MB/size_mb:>14.1f}x")
    print("-" * 60)
    print(f"Foundry is {results['ratio_vs_electron']}x smaller than Electron")
    print()

    return results


if __name__ == "__main__":
    results = main()
    # Write results for aggregation
    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "_size.json")
    with open(out, "w") as f:
        json.dump(results, f, indent=2)
