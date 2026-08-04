use foundry_runtime::{codegen, css, events, html, js, layout, render, scene, text};

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

#[derive(Parser)]
#[command(name = "foundry", about = "Cast web into native metal.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile HTML/CSS/JS to a native GPU-rendered window
    Dev {
        /// Path to HTML file
        path: PathBuf,
    },
    /// Build a native binary from HTML/CSS/JS
    Build {
        /// Path to HTML file
        path: PathBuf,
        /// Output binary path
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Dev { path } => run_dev(path),
        Commands::Build { path, output } => run_build(path, output),
    }
}

fn load_html(path: &PathBuf) -> (scene::SceneGraph, Vec<String>) {
    let html_source = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("foundry: cannot read {}: {}", path.display(), e);
        std::process::exit(1);
    });

    let base_dir = path.parent().unwrap_or(std::path::Path::new("."));

    // Single parse: extract scene, styles, and scripts from one DOM walk
    let (mut scene, inline_styles, inline_scripts, ext_style_paths, ext_script_paths) =
        html::parse_html_full(&html_source);

    // Apply CSS: inline <style> blocks + external <link> stylesheets
    let mut all_css = inline_styles;
    for style_path in &ext_style_paths {
        let resolved = base_dir.join(style_path);
        match std::fs::read_to_string(&resolved) {
            Ok(content) => all_css.push(content),
            Err(_) => eprintln!("foundry: cannot read stylesheet: {}", resolved.display()),
        }
    }
    for style_text in &all_css {
        let (rules, kfs) = css::parse_stylesheet_with_keyframes(style_text);
        scene.keyframes.extend(kfs);
        css::apply_styles(&mut scene, &rules);
    }

    // Collect JS: inline <script> blocks + external <script src> files
    let mut scripts = inline_scripts;
    for script_path in &ext_script_paths {
        let resolved = base_dir.join(script_path);
        match std::fs::read_to_string(&resolved) {
            Ok(content) => scripts.push(content),
            Err(_) => eprintln!("foundry: cannot read script: {}", resolved.display()),
        }
    }

    println!(
        "foundry: loaded {} nodes from {}",
        scene.node_count(),
        path.display()
    );

    (scene, scripts)
}

struct App {
    path: PathBuf,
    window: Option<Arc<Window>>,
    renderer: Option<render::Renderer>,
    text_engine: Option<text::TextEngine>,
    scene: scene::SceneGraph,
    scripts: Vec<String>,
    layout_engine: layout::LayoutEngine,
    event_system: events::EventSystem,
    js_engine: Option<js::JsEngine>,
    needs_relayout: bool,
    scripts_executed: bool,
    cursor_pos: (f32, f32),
    file_changed: Arc<AtomicBool>,
}

impl App {
    fn new(path: PathBuf, file_changed: Arc<AtomicBool>) -> Self {
        let (scene, scripts) = load_html(&path);

        Self {
            path,
            window: None,
            renderer: None,
            text_engine: None,
            scene,
            scripts,
            layout_engine: layout::LayoutEngine::new(),
            event_system: events::EventSystem::new(),
            js_engine: None,
            needs_relayout: true,
            scripts_executed: false,
            cursor_pos: (0.0, 0.0),
            file_changed,
        }
    }

    fn reload(&mut self) {
        println!("foundry: reloading {}", self.path.display());
        let (scene, scripts) = load_html(&self.path);
        self.scene = scene;
        self.scripts = scripts;
        self.scripts_executed = false;
        self.needs_relayout = true;
        self.event_system = events::EventSystem::new();
        self.js_engine = Some(js::JsEngine::new());
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title(format!("Foundry - {}", self.path.display()))
                        .with_inner_size(winit::dpi::LogicalSize::new(1024.0, 768.0)),
                )
                .expect("failed to create window"),
        );

        let renderer = pollster::block_on(render::Renderer::new(window.clone()));
        let te = text::TextEngine::new(&renderer.device, &renderer.queue, renderer.format);

        self.window = Some(window.clone());
        self.text_engine = Some(te);
        self.renderer = Some(renderer);
        self.js_engine = Some(js::JsEngine::new());
        self.needs_relayout = true;
        window.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                    self.needs_relayout = true;
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let (x, y) = self.cursor_pos;
                let app_event = events::AppEvent::Click(x, y);
                let results = self.event_system.handle(&app_event, &mut self.scene);
                if !results.is_empty() {
                    self.execute_event_handlers(&results);
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = (position.x as f32, position.y as f32);
                let old_hover = self.event_system.hover_node;
                let app_event = events::AppEvent::MouseMove(position.x as f32, position.y as f32);
                let results = self.event_system.handle(&app_event, &mut self.scene);
                if !results.is_empty() {
                    self.execute_event_handlers(&results);
                }
                // If hover changed, trigger relayout for hover style changes
                if self.event_system.hover_node != old_hover {
                    self.needs_relayout = true;
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (x, y),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => (pos.x as f32, pos.y as f32),
                };
                let app_event =
                    events::AppEvent::Scroll(self.cursor_pos.0, self.cursor_pos.1, dx, -dy);
                self.event_system.handle(&app_event, &mut self.scene);
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if self.needs_relayout {
                    if let Some(renderer) = &self.renderer {
                        let (vw, vh) = renderer.viewport_size();
                        self.layout_engine.compute(&mut self.scene, vw, vh);
                        self.needs_relayout = false;
                    }

                    // Execute scripts once after first layout
                    if !self.scripts_executed {
                        self.scripts_executed = true;
                        let scripts = self.scripts.clone();
                        for script in &scripts {
                            if let Some(js) = &mut self.js_engine {
                                if let Err(e) = js.execute(script) {
                                    eprintln!("foundry: {}", e);
                                }
                                let mutations = js.take_mutations();
                                if !mutations.is_empty() {
                                    js.apply_mutations(&mut self.scene, &mutations);
                                    if let Some(renderer) = &self.renderer {
                                        let (vw, vh) = renderer.viewport_size();
                                        self.layout_engine.compute(&mut self.scene, vw, vh);
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(renderer) = &mut self.renderer {
                    match renderer.render(&self.scene, self.text_engine.as_mut()) {
                        Ok(_) => {}
                        Err(e) => {
                            let msg = e.to_string();
                            if msg.contains("lost") || msg.contains("Lost") {
                                let (vw, vh) = renderer.viewport_size();
                                renderer.resize(vw as u32, vh as u32);
                            } else if msg.contains("OutOfMemory") {
                                event_loop.exit();
                            } else {
                                eprintln!("foundry: render error: {}", e);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Hot reload
        if self.file_changed.swap(false, Ordering::Relaxed) {
            self.reload();
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }

        // Tick transitions + animations with frame pacing
        if self.scene.tick_animations() {
            self.needs_relayout = true;
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            // Request next frame at ~60fps to avoid CPU spin
            event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                std::time::Instant::now() + std::time::Duration::from_millis(16),
            ));
        } else {
            // No animations active — wait for events (no CPU usage)
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        }
    }
}

impl App {
    fn execute_event_handlers(&mut self, results: &[events::EventResult]) {
        for result in results {
            if let Some(js) = &mut self.js_engine {
                if let Err(e) = js.execute(&result.handler_code) {
                    eprintln!("foundry: event handler error: {}", e);
                }
                let mutations = js.take_mutations();
                if !mutations.is_empty() {
                    js.apply_mutations(&mut self.scene, &mutations);
                    self.needs_relayout = true;
                }
            }
        }
    }
}

fn run_dev(path: PathBuf) {
    println!("foundry: dev mode - {}", path.display());
    println!("foundry: watching for changes (hot reload enabled)");

    let file_changed = Arc::new(AtomicBool::new(false));

    // Set up file watcher for hot reload
    let watch_flag = file_changed.clone();
    let _watch_path = path.clone();
    let watch_dir = path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    std::thread::spawn(move || {
        use notify::{Event, EventKind, RecursiveMode, Watcher};
        let flag = watch_flag;
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        // Only reload for HTML/CSS/JS files
                        let relevant = event.paths.iter().any(|p| {
                            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                            matches!(ext, "html" | "css" | "js" | "htm")
                        });
                        if relevant {
                            flag.store(true, Ordering::Relaxed);
                        }
                    }
                    _ => {}
                }
            }
        })
        .expect("failed to create file watcher");

        watcher
            .watch(&watch_dir, RecursiveMode::Recursive)
            .expect("failed to watch directory");

        // Keep watcher alive
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
    });

    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = App::new(path, file_changed);

    event_loop.run_app(&mut app).expect("event loop failed");
}

fn run_build(path: PathBuf, output: Option<PathBuf>) {
    let stem = path.file_stem().unwrap().to_str().unwrap();
    let out = output.unwrap_or_else(|| {
        if cfg!(windows) {
            PathBuf::from(format!("{}.exe", stem))
        } else {
            PathBuf::from(stem)
        }
    });

    println!("foundry: compiling {} -> {}", path.display(), out.display());

    // Read the HTML source
    let html_source = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("foundry: cannot read {}: {}", path.display(), e);
        std::process::exit(1);
    });

    let base_dir = path.parent().unwrap_or(std::path::Path::new("."));
    let (scene, _scripts) = load_html(&path);
    println!("  scene: {} nodes", scene.node_count());

    // Inline external CSS/JS into the HTML so the standalone binary has everything
    let mut final_html = html_source.clone();
    let (_, _, _, ext_style_paths, ext_script_paths) = html::parse_html_full(&html_source);
    for style_path in &ext_style_paths {
        let resolved = base_dir.join(style_path);
        if let Ok(content) = std::fs::read_to_string(&resolved) {
            let inline = format!("<style>\n{}\n</style>", content);
            let link_tag = format!("<link rel=\"stylesheet\" href=\"{}\">", style_path);
            // Try to replace the link tag; if not found, append before </head>
            if final_html.contains(&link_tag) {
                final_html = final_html.replace(&link_tag, &inline);
            } else {
                final_html = final_html.replace("</head>", &format!("{}\n</head>", inline));
            }
            println!("  inlined: {}", resolved.display());
        }
    }
    for script_path in &ext_script_paths {
        let resolved = base_dir.join(script_path);
        if let Ok(content) = std::fs::read_to_string(&resolved) {
            let inline = format!("<script>\n{}\n</script>", content);
            let script_tag = format!("<script src=\"{}\"></script>", script_path);
            if final_html.contains(&script_tag) {
                final_html = final_html.replace(&script_tag, &inline);
            } else {
                final_html = final_html.replace("</body>", &format!("{}\n</body>", inline));
            }
            println!("  inlined: {}", resolved.display());
        }
    }

    let title = stem.to_string();

    let build_dir = std::env::temp_dir().join(format!("foundry_{}", stem));
    let src_dir = build_dir.join("src");
    let _ = std::fs::remove_dir_all(&build_dir);
    std::fs::create_dir_all(&src_dir).expect("failed to create build directory");

    // Find the foundry crate path (this binary's crate)
    let foundry_crate = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .and_then(|p| {
            // Walk up from target/debug or target/release to find Cargo.toml
            let mut dir = p;
            for _ in 0..5 {
                if dir.join("Cargo.toml").exists() {
                    let content = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
                    if content.contains("foundry_runtime") {
                        return Some(dir);
                    }
                }
                dir = dir.parent()?.to_path_buf();
            }
            None
        });

    // Use the crate path if found, otherwise fall back to crates.io
    let dep_line = codegen::foundry_dep_line(foundry_crate.as_deref().and_then(|p| p.to_str()));

    // Write Cargo.toml for the generated project
    let cargo_toml = codegen::generate_cargo_toml(stem, &dep_line);
    std::fs::write(build_dir.join("Cargo.toml"), cargo_toml).expect("failed to write Cargo.toml");

    // Write main.rs that embeds the HTML
    let main_rs = codegen::generate_main_rs(&final_html, &title);
    std::fs::write(src_dir.join("main.rs"), &main_rs).expect("failed to write main.rs");

    println!("  compiling native binary...");

    // Run cargo build --release
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = std::process::Command::new(&cargo)
        .args(["build", "--release", "-j2", "--manifest-path"])
        .arg(build_dir.join("Cargo.toml").to_str().unwrap())
        .status();

    match status {
        Ok(s) if s.success() => {
            // Copy the built binary to the output path
            let built_binary = if cfg!(windows) {
                build_dir.join(format!("target/release/{}.exe", stem))
            } else {
                build_dir.join(format!("target/release/{}", stem))
            };

            if built_binary.exists() {
                std::fs::copy(&built_binary, &out).expect("failed to copy binary");
                let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
                let size_mb = size as f64 / (1024.0 * 1024.0);
                println!();
                println!("  output: {} ({:.1} MB)", out.display(), size_mb);
                println!("  done.");
            } else {
                eprintln!(
                    "foundry: build succeeded but binary not found at {:?}",
                    built_binary
                );
                std::process::exit(1);
            }
        }
        Ok(s) => {
            eprintln!("foundry: compilation failed (exit code: {:?})", s.code());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("foundry: failed to run cargo: {}", e);
            eprintln!("  ensure Rust toolchain is installed: https://rustup.rs");
            std::process::exit(1);
        }
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&build_dir);
}
