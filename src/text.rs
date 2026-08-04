use crate::scene::*;
use glyphon::{
    Attrs, Buffer, Cache, Color as GlyphonColor, Family, FontSystem, Metrics, Resolution, Shaping,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport, Weight,
};

pub struct TextEngine {
    pub font_system: FontSystem,
    pub cache: SwashCache,
    pub atlas: TextAtlas,
    pub text_renderer: TextRenderer,
    pub buffers: Vec<(NodeId, Buffer)>,
    pub viewport: Viewport,
}

impl TextEngine {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let font_system = FontSystem::new();
        let cache = SwashCache::new();
        let glyph_cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &glyph_cache, format);
        let text_renderer =
            TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);
        let viewport = Viewport::new(device, &glyph_cache);

        Self {
            font_system,
            cache,
            atlas,
            text_renderer,
            buffers: Vec::new(),
            viewport,
        }
    }

    pub fn prepare(
        &mut self,
        scene: &SceneGraph,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        viewport_width: u32,
        viewport_height: u32,
    ) {
        self.buffers.clear();

        if let Some(root) = scene.root {
            self.collect_text_nodes(scene, root);
        }

        self.viewport.update(
            queue,
            Resolution {
                width: viewport_width,
                height: viewport_height,
            },
        );

        let text_areas: Vec<TextArea> = self
            .buffers
            .iter()
            .map(|(node_id, buffer)| {
                let node = scene.get(*node_id);
                let color = &node.style.color;
                let r = (color.r * 255.0) as u8;
                let g = (color.g * 255.0) as u8;
                let b = (color.b * 255.0) as u8;
                let a = (color.a * 255.0) as u8;

                TextArea {
                    buffer,
                    left: node.layout.x,
                    top: node.layout.y,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: viewport_width as i32,
                        bottom: viewport_height as i32,
                    },
                    default_color: GlyphonColor::rgba(r, g, b, a),
                    custom_glyphs: &[],
                }
            })
            .collect();

        self.text_renderer
            .prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                text_areas,
                &mut self.cache,
            )
            .ok();
    }

    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        self.text_renderer
            .render(&self.atlas, &self.viewport, pass)
            .ok();
    }

    fn collect_text_nodes(&mut self, scene: &SceneGraph, node_id: NodeId) {
        let node = scene.get(node_id);

        if node.style.display == Display::None {
            return;
        }

        if node.kind == ElementKind::Text {
            if let Some(text) = &node.text_content {
                let parent_style = node
                    .parent
                    .map(|p| &scene.get(p).style)
                    .unwrap_or(&node.style);

                let font_size = parent_style.font_size;
                let line_height = font_size * parent_style.line_height;

                let mut buffer =
                    Buffer::new(&mut self.font_system, Metrics::new(font_size, line_height));

                let weight = if parent_style.font_weight >= 700 {
                    Weight::BOLD
                } else if parent_style.font_weight >= 500 {
                    Weight::MEDIUM
                } else {
                    Weight::NORMAL
                };

                let family = if parent_style.font_family.is_empty() {
                    Family::SansSerif
                } else {
                    Family::Name(&parent_style.font_family)
                };

                let attrs = Attrs::new().family(family).weight(weight);

                // Never constrain single-line text width. Let glyphon render
                // the full text without wrapping. Overflow is better than clipping.
                buffer.set_size(&mut self.font_system, None, None);
                buffer.set_text(&mut self.font_system, text, &attrs, Shaping::Advanced, None);
                buffer.shape_until_scroll(&mut self.font_system, false);

                self.buffers.push((node_id, buffer));
            }
        }

        let children: Vec<NodeId> = node.children.clone();
        for child_id in children {
            self.collect_text_nodes(scene, child_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_gpu::with_headless_device;

    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

    /// `<div><p style="font-size:{size}px">{text}</p></div>`
    fn scene_with_text(text: &str, font_size: f32) -> (SceneGraph, NodeId) {
        let mut scene = SceneGraph::new();
        let root = scene.add_node(ElementKind::from_tag("div"), "div".to_string());
        let p = scene.add_node(ElementKind::from_tag("p"), "p".to_string());
        scene.add_child(root, p);
        scene.get_mut(p).style.font_size = font_size;
        let t = scene.add_node(ElementKind::Text, "#text".to_string());
        scene.get_mut(t).text_content = Some(text.to_string());
        scene.add_child(p, t);
        (scene, t)
    }

    /// Total shaped advance width of the first prepared buffer.
    fn shaped_width(engine: &TextEngine) -> f32 {
        engine.buffers[0]
            .1
            .layout_runs()
            .map(|run| run.line_w)
            .fold(0.0, f32::max)
    }

    #[test]
    fn text_nodes_are_collected_and_shaped_into_glyphs() {
        with_headless_device(
            "text_nodes_are_collected_and_shaped_into_glyphs",
            |device, queue| {
                let mut engine = TextEngine::new(device, queue, FORMAT);
                let (scene, t) = scene_with_text("hello", 16.0);

                engine.prepare(&scene, device, queue, 800, 600);

                assert_eq!(engine.buffers.len(), 1);
                assert_eq!(engine.buffers[0].0, t);
                let glyphs: usize = engine.buffers[0]
                    .1
                    .layout_runs()
                    .map(|r| r.glyphs.len())
                    .sum();
                assert_eq!(glyphs, 5, "one glyph per ASCII character");
                assert!(shaped_width(&engine) > 0.0);
            },
        );
    }

    #[test]
    fn shaped_width_scales_with_the_font_size() {
        with_headless_device("shaped_width_scales_with_the_font_size", |device, queue| {
            let mut engine = TextEngine::new(device, queue, FORMAT);

            let (small, _) = scene_with_text("hello world", 16.0);
            engine.prepare(&small, device, queue, 800, 600);
            let w16 = shaped_width(&engine);

            let (large, _) = scene_with_text("hello world", 32.0);
            engine.prepare(&large, device, queue, 800, 600);
            let w32 = shaped_width(&engine);

            assert!(w16 > 0.0 && w32 > 0.0, "{w16} {w32}");
            // doubling the em size roughly doubles the advance
            let ratio = w32 / w16;
            assert!((1.8..=2.2).contains(&ratio), "ratio {ratio}");
        });
    }

    #[test]
    fn shaped_width_grows_with_the_string() {
        with_headless_device("shaped_width_grows_with_the_string", |device, queue| {
            let mut engine = TextEngine::new(device, queue, FORMAT);

            let (short, _) = scene_with_text("hi", 16.0);
            engine.prepare(&short, device, queue, 800, 600);
            let short_w = shaped_width(&engine);

            let (long, _) = scene_with_text("hi there, this is longer", 16.0);
            engine.prepare(&long, device, queue, 800, 600);
            let long_w = shaped_width(&engine);

            assert!(long_w > short_w, "{long_w} !> {short_w}");
        });
    }

    #[test]
    fn line_metrics_come_from_the_parent_font_size_and_line_height() {
        with_headless_device(
            "line_metrics_come_from_the_parent_font_size_and_line_height",
            |device, queue| {
                let mut engine = TextEngine::new(device, queue, FORMAT);
                let (mut scene, _) = scene_with_text("hello", 20.0);
                let p = scene.get(scene.root.unwrap()).children[0];
                scene.get_mut(p).style.line_height = 1.5;

                engine.prepare(&scene, device, queue, 800, 600);

                let run_height = engine.buffers[0]
                    .1
                    .layout_runs()
                    .map(|r| r.line_height)
                    .next()
                    .expect("no layout runs");
                assert!((run_height - 30.0).abs() < 0.01, "line height {run_height}");
            },
        );
    }

    #[test]
    fn long_text_is_not_wrapped_into_extra_lines() {
        with_headless_device(
            "long_text_is_not_wrapped_into_extra_lines",
            |device, queue| {
                let mut engine = TextEngine::new(device, queue, FORMAT);
                let (scene, _) = scene_with_text(
                    "a very long single line of text that would wrap in a narrow box",
                    16.0,
                );

                // the surface is deliberately narrower than the shaped text
                engine.prepare(&scene, device, queue, 100, 100);

                assert_eq!(engine.buffers[0].1.layout_runs().count(), 1);
                assert!(shaped_width(&engine) > 100.0);
            },
        );
    }

    #[test]
    fn display_none_text_is_not_prepared() {
        with_headless_device("display_none_text_is_not_prepared", |device, queue| {
            let mut engine = TextEngine::new(device, queue, FORMAT);
            let (mut scene, _) = scene_with_text("hello", 16.0);
            let p = scene.get(scene.root.unwrap()).children[0];
            scene.get_mut(p).style.display = Display::None;

            engine.prepare(&scene, device, queue, 800, 600);

            assert!(engine.buffers.is_empty());
        });
    }

    #[test]
    fn every_text_node_in_the_tree_gets_its_own_buffer() {
        with_headless_device(
            "every_text_node_in_the_tree_gets_its_own_buffer",
            |device, queue| {
                let mut engine = TextEngine::new(device, queue, FORMAT);
                let mut scene = SceneGraph::new();
                let root = scene.add_node(ElementKind::from_tag("div"), "div".to_string());
                for word in ["one", "two", "three"] {
                    let p = scene.add_node(ElementKind::from_tag("p"), "p".to_string());
                    scene.add_child(root, p);
                    let t = scene.add_node(ElementKind::Text, "#text".to_string());
                    scene.get_mut(t).text_content = Some(word.to_string());
                    scene.add_child(p, t);
                }

                engine.prepare(&scene, device, queue, 800, 600);

                assert_eq!(engine.buffers.len(), 3);
                // prepare is idempotent -- buffers are rebuilt, not appended
                engine.prepare(&scene, device, queue, 800, 600);
                assert_eq!(engine.buffers.len(), 3);
            },
        );
    }

    #[test]
    fn an_empty_scene_prepares_no_text() {
        with_headless_device("an_empty_scene_prepares_no_text", |device, queue| {
            let mut engine = TextEngine::new(device, queue, FORMAT);
            engine.prepare(&SceneGraph::new(), device, queue, 800, 600);
            assert!(engine.buffers.is_empty());
        });
    }
}
