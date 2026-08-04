pub mod codegen;
pub mod css;
pub mod events;
pub mod html;
pub mod js;
pub mod layout;
pub mod render;
pub mod scene;
pub mod text;

#[cfg(test)]
mod test_gpu;

use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

/// Entry point for compiled Foundry binaries -- runs embedded HTML as a native GPU window.
pub fn run_embedded(html: &str, title: &str) {
    env_logger::init();

    let (scene_graph, scripts) = parse_embedded(html);

    println!("foundry: {} nodes", scene_graph.node_count());

    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = EmbeddedApp {
        title: title.to_string(),
        window: None,
        renderer: None,
        text_engine: None,
        scene: scene_graph,
        scripts,
        layout_engine: layout::LayoutEngine::new(),
        event_system: events::EventSystem::new(),
        js_engine: None,
        needs_relayout: true,
        scripts_executed: false,
        cursor_pos: (0.0, 0.0),
    };

    event_loop.run_app(&mut app).expect("event loop failed");
}

fn parse_embedded(html_source: &str) -> (scene::SceneGraph, Vec<String>) {
    let (mut scene_graph, inline_styles, inline_scripts, _, _) = html::parse_html_full(html_source);

    for style_text in &inline_styles {
        let (rules, kfs) = css::parse_stylesheet_with_keyframes(style_text);
        scene_graph.keyframes.extend(kfs);
        css::apply_styles(&mut scene_graph, &rules);
    }

    (scene_graph, inline_scripts)
}

struct EmbeddedApp {
    title: String,
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
}

impl ApplicationHandler for EmbeddedApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title(&self.title)
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
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
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
                    self.execute_handlers(&results);
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
                    self.execute_handlers(&results);
                }
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
                            eprintln!("foundry: render error: {}", e);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.scene.tick_animations() {
            self.needs_relayout = true;
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                std::time::Instant::now() + std::time::Duration::from_millis(16),
            ));
        } else {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        }
    }
}

impl EmbeddedApp {
    fn execute_handlers(&mut self, results: &[events::EventResult]) {
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
