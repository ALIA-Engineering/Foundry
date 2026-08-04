use wgpu::util::DeviceExt;

use crate::scene::*;

/// One corner of a rounded-rect quad. The fragment shader reconstructs the box
/// from `rect` + `border_radius` and evaluates a rounded-rect SDF per pixel, so
/// every vertex of a quad carries identical rect/radius/border data.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
    pub border_color: [f32; 4],
    pub rect: [f32; 4], // x, y, width, height in pixels
    pub border_radius: [f32; 4],
    pub border_width: f32,
    pub _padding: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Viewport {
    size: [f32; 2],
    _pad: [f32; 2],
}

/// Walk the scene graph and build the draw commands (vertex + index buffers)
/// for every visible node that paints a background or a border.
///
/// This is the whole "scene graph -> draw commands" step and is deliberately
/// free of GPU state so it can be tested without a device or a surface.
pub fn build_quads(scene: &SceneGraph) -> (Vec<QuadVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    if let Some(root) = scene.root {
        collect_quads(scene, root, &mut vertices, &mut indices, 0.0, 0.0);
    }
    (vertices, indices)
}

fn collect_quads(
    scene: &SceneGraph,
    node_id: NodeId,
    vertices: &mut Vec<QuadVertex>,
    indices: &mut Vec<u32>,
    scroll_x: f32,
    scroll_y: f32,
) {
    let node = scene.get(node_id);

    if node.style.display == Display::None {
        return;
    }

    let layout = &node.layout;
    let x = layout.x - scroll_x;
    let y = layout.y - scroll_y;
    let w = layout.width;
    let h = layout.height;

    if w <= 0.0 || h <= 0.0 {
        let sx = scroll_x + node.scroll_offset.0;
        let sy = scroll_y + node.scroll_offset.1;
        for &child_id in &node.children {
            collect_quads(scene, child_id, vertices, indices, sx, sy);
        }
        return;
    }

    let bg = node.style.background_color;
    let has_bg = bg.a > 0.0;
    let has_border = node.style.border_width.iter().any(|&w| w > 0.0);

    if has_bg || has_border {
        let base_idx = vertices.len() as u32;

        let color = [bg.r, bg.g, bg.b, bg.a * node.style.opacity];
        let bc = node.style.border_color;
        let border_color = [bc.r, bc.g, bc.b, bc.a];
        let rect = [x, y, w, h];
        let border_radius = node.style.border_radius;
        let border_width = node.style.border_width[0];

        let corners = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];

        for pos in corners {
            vertices.push(QuadVertex {
                position: pos,
                color,
                border_color,
                rect,
                border_radius,
                border_width,
                _padding: [0.0; 3],
            });
        }

        indices.extend_from_slice(&[
            base_idx,
            base_idx + 1,
            base_idx + 2,
            base_idx,
            base_idx + 2,
            base_idx + 3,
        ]);
    }

    let sx = scroll_x + node.scroll_offset.0;
    let sy = scroll_y + node.scroll_offset.1;
    for &child_id in &node.children {
        collect_quads(scene, child_id, vertices, indices, sx, sy);
    }
}

/// Pick the framebuffer clear colour: the root background if it is opaque
/// enough to matter, else the first child (`<body>`) that has one, else white.
pub fn clear_color(scene: &SceneGraph) -> wgpu::Color {
    let white = wgpu::Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    let to_wgpu = |c: &Color| wgpu::Color {
        r: c.r as f64,
        g: c.g as f64,
        b: c.b as f64,
        a: c.a as f64,
    };

    let Some(root) = scene.root else {
        return white;
    };
    let bg = &scene.get(root).style.background_color;
    if bg.a > 0.0 {
        return to_wgpu(bg);
    }
    for &child in &scene.get(root).children {
        let cbg = &scene.get(child).style.background_color;
        if cbg.a > 0.0 {
            return to_wgpu(cbg);
        }
    }
    white
}

/// WGSL source for the rounded-rect SDF quad pipeline. Exposed so tests can
/// compile it against a headless device.
pub fn quad_shader_source() -> &'static str {
    QUAD_SHADER
}

/// Bind group layout for the viewport uniform consumed by the quad shader.
pub fn create_viewport_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("viewport_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

/// Build the rounded-rect SDF quad pipeline. Split out of [`Renderer::new`] so
/// it can be created against a headless device in tests.
pub fn create_quad_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("quad_shader"),
        source: wgpu::ShaderSource::Wgsl(QUAD_SHADER.into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("quad_pipeline_layout"),
        bind_group_layouts: &[Some(bind_group_layout)],
        ..Default::default()
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("quad_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<QuadVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![
                    0 => Float32x2,  // position
                    1 => Float32x4,  // color
                    2 => Float32x4,  // border_color
                    3 => Float32x4,  // rect
                    4 => Float32x4,  // border_radius
                    5 => Float32,    // border_width
                ],
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    quad_pipeline: wgpu::RenderPipeline,
    viewport_buffer: wgpu::Buffer,
    viewport_bind_group: wgpu::BindGroup,
    viewport: [f32; 2],
    pub format: wgpu::TextureFormat,
}

impl Renderer {
    pub async fn new(window: std::sync::Arc<winit::window::Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::default();

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("foundry"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::default(),
            })
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Viewport uniform buffer
        let viewport_data = Viewport {
            size: [size.width as f32, size.height as f32],
            _pad: [0.0; 2],
        };
        let viewport_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("viewport_uniform"),
            contents: bytemuck::cast_slice(&[viewport_data]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = create_viewport_bind_group_layout(&device);

        let viewport_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: viewport_buffer.as_entire_binding(),
            }],
        });

        let quad_pipeline = create_quad_pipeline(&device, &bind_group_layout, format);

        Self {
            device,
            queue,
            surface,
            config,
            quad_pipeline,
            viewport_buffer,
            viewport_bind_group,
            viewport: [size.width as f32, size.height as f32],
            format,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.viewport = [width as f32, height as f32];

            let viewport_data = Viewport {
                size: self.viewport,
                _pad: [0.0; 2],
            };
            self.queue.write_buffer(
                &self.viewport_buffer,
                0,
                bytemuck::cast_slice(&[viewport_data]),
            );
        }
    }

    pub fn viewport_size(&self) -> (f32, f32) {
        (self.viewport[0], self.viewport[1])
    }

    pub fn render(
        &mut self,
        scene: &SceneGraph,
        mut text_engine: Option<&mut crate::text::TextEngine>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(tex)
            | wgpu::CurrentSurfaceTexture::Suboptimal(tex) => tex,
            other => {
                return Err(format!("surface error: {:?}", other).into());
            }
        };
        let view = output.texture.create_view(&Default::default());

        if let Some(te) = text_engine.as_mut() {
            te.prepare(
                scene,
                &self.device,
                &self.queue,
                self.config.width,
                self.config.height,
            );
        }

        let (vertices, indices) = build_quads(scene);

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("quad_vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("quad_indices"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });

        // Use root node's background color as clear color (or white fallback)
        let clear_color = clear_color(scene);

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if !indices.is_empty() {
                pass.set_pipeline(&self.quad_pipeline);
                pass.set_bind_group(0, &self.viewport_bind_group, &[]);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
            }

            if let Some(te) = text_engine.as_ref() {
                te.render(&mut pass);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

const QUAD_SHADER: &str = r#"
struct Viewport {
    size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> viewport: Viewport;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) border_color: vec4<f32>,
    @location(3) rect: vec4<f32>,
    @location(4) border_radius: vec4<f32>,
    @location(5) border_width: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) border_color: vec4<f32>,
    @location(2) pixel_pos: vec2<f32>,
    @location(3) rect: vec4<f32>,
    @location(4) border_radius: vec4<f32>,
    @location(5) border_width: f32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let x = (in.position.x / viewport.size.x) * 2.0 - 1.0;
    let y = 1.0 - (in.position.y / viewport.size.y) * 2.0;
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.color = in.color;
    out.border_color = in.border_color;
    out.pixel_pos = in.position;
    out.rect = in.rect;
    out.border_radius = in.border_radius;
    out.border_width = in.border_width;
    return out;
}

fn rounded_rect_sdf(pixel: vec2<f32>, rect: vec4<f32>, radius: vec4<f32>) -> f32 {
    let center = vec2<f32>(rect.x + rect.z * 0.5, rect.y + rect.w * 0.5);
    let half_size = vec2<f32>(rect.z * 0.5, rect.w * 0.5);
    let p = pixel - center;

    var r: f32;
    if p.x < 0.0 {
        if p.y < 0.0 { r = radius.x; }
        else { r = radius.w; }
    } else {
        if p.y < 0.0 { r = radius.y; }
        else { r = radius.z; }
    }

    let q = abs(p) - half_size + vec2<f32>(r, r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0, 0.0))) - r;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let d = rounded_rect_sdf(in.pixel_pos, in.rect, in.border_radius);

    let aa = 1.0;
    let alpha = 1.0 - smoothstep(-aa, aa, d);

    if alpha < 0.001 {
        discard;
    }

    if in.border_width > 0.0 {
        let inner_d = rounded_rect_sdf(
            in.pixel_pos,
            vec4<f32>(
                in.rect.x + in.border_width,
                in.rect.y + in.border_width,
                in.rect.z - in.border_width * 2.0,
                in.rect.w - in.border_width * 2.0,
            ),
            max(in.border_radius - vec4<f32>(in.border_width), vec4<f32>(0.0)),
        );

        let border_alpha = smoothstep(-aa, aa, inner_d);
        let color = mix(in.color, in.border_color, border_alpha);
        return vec4<f32>(color.rgb, color.a * alpha);
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_gpu::with_headless_device;

    fn node(scene: &mut SceneGraph, tag: &str, rect: (f32, f32, f32, f32)) -> NodeId {
        let id = scene.add_node(ElementKind::from_tag(tag), tag.to_string());
        scene.get_mut(id).layout = LayoutRect {
            x: rect.0,
            y: rect.1,
            width: rect.2,
            height: rect.3,
        };
        id
    }

    fn opaque(scene: &mut SceneGraph, id: NodeId, r: u8, g: u8, b: u8) {
        scene.get_mut(id).style.background_color = Color::from_rgba(r, g, b, 1.0);
    }

    // ---- scene graph -> draw commands ----

    #[test]
    fn a_painted_node_becomes_one_quad_of_four_vertices_and_two_triangles() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "div", (10.0, 20.0, 100.0, 50.0));
        opaque(&mut scene, root, 255, 0, 0);

        let (vertices, indices) = build_quads(&scene);

        assert_eq!(vertices.len(), 4);
        assert_eq!(indices, vec![0, 1, 2, 0, 2, 3]);
        // corners are emitted clockwise from the top-left
        let positions: Vec<[f32; 2]> = vertices.iter().map(|v| v.position).collect();
        assert_eq!(
            positions,
            vec![[10.0, 20.0], [110.0, 20.0], [110.0, 70.0], [10.0, 70.0]]
        );
        // every vertex carries the whole rect so the SDF can be evaluated per pixel
        assert!(vertices.iter().all(|v| v.rect == [10.0, 20.0, 100.0, 50.0]));
    }

    #[test]
    fn a_fully_transparent_node_emits_nothing() {
        let mut scene = SceneGraph::new();
        node(&mut scene, "div", (0.0, 0.0, 100.0, 100.0));

        let (vertices, indices) = build_quads(&scene);

        assert!(vertices.is_empty());
        assert!(indices.is_empty());
    }

    #[test]
    fn a_transparent_node_with_a_border_still_emits_a_quad() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "div", (0.0, 0.0, 100.0, 100.0));
        scene.get_mut(root).style.border_width = [2.0; 4];
        scene.get_mut(root).style.border_color = Color::from_rgba(0, 0, 255, 1.0);

        let (vertices, _) = build_quads(&scene);

        assert_eq!(vertices.len(), 4);
        assert_eq!(vertices[0].border_width, 2.0);
        assert_eq!(vertices[0].border_color, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(vertices[0].color[3], 0.0, "background stays transparent");
    }

    #[test]
    fn a_transparent_parent_does_not_hide_its_painted_children() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "div", (0.0, 0.0, 200.0, 200.0));
        let a = node(&mut scene, "div", (0.0, 0.0, 10.0, 10.0));
        let b = node(&mut scene, "div", (20.0, 0.0, 10.0, 10.0));
        scene.add_child(root, a);
        scene.add_child(root, b);
        opaque(&mut scene, a, 255, 0, 0);
        opaque(&mut scene, b, 0, 255, 0);

        let (vertices, indices) = build_quads(&scene);

        assert_eq!(vertices.len(), 8);
        assert_eq!(indices.len(), 12);
        // indices of the second quad are offset by the first quad's vertices
        assert_eq!(&indices[6..], &[4, 5, 6, 4, 6, 7]);
    }

    #[test]
    fn children_are_drawn_after_their_parent() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "div", (0.0, 0.0, 200.0, 200.0));
        let child = node(&mut scene, "div", (0.0, 0.0, 10.0, 10.0));
        scene.add_child(root, child);
        opaque(&mut scene, root, 255, 0, 0);
        opaque(&mut scene, child, 0, 255, 0);

        let (vertices, _) = build_quads(&scene);

        assert_eq!(vertices[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(vertices[4].color, [0.0, 1.0, 0.0, 1.0]);
    }

    #[test]
    fn display_none_removes_the_whole_subtree_from_the_draw_list() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "div", (0.0, 0.0, 200.0, 200.0));
        let hidden = node(&mut scene, "div", (0.0, 0.0, 50.0, 50.0));
        let inside = node(&mut scene, "div", (0.0, 0.0, 10.0, 10.0));
        scene.add_child(root, hidden);
        scene.add_child(hidden, inside);
        opaque(&mut scene, root, 255, 0, 0);
        opaque(&mut scene, hidden, 0, 255, 0);
        opaque(&mut scene, inside, 0, 0, 255);
        scene.get_mut(hidden).style.display = Display::None;

        let (vertices, _) = build_quads(&scene);

        assert_eq!(vertices.len(), 4, "only the root should be drawn");
    }

    #[test]
    fn a_zero_sized_node_is_skipped_but_its_children_are_not() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "div", (0.0, 0.0, 0.0, 0.0));
        let child = node(&mut scene, "div", (5.0, 5.0, 10.0, 10.0));
        scene.add_child(root, child);
        opaque(&mut scene, root, 255, 0, 0);
        opaque(&mut scene, child, 0, 255, 0);

        let (vertices, _) = build_quads(&scene);

        assert_eq!(vertices.len(), 4);
        assert_eq!(vertices[0].color, [0.0, 1.0, 0.0, 1.0]);
    }

    #[test]
    fn opacity_is_folded_into_the_vertex_alpha() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "div", (0.0, 0.0, 10.0, 10.0));
        scene.get_mut(root).style.background_color = Color::from_rgba(255, 0, 0, 0.5);
        scene.get_mut(root).style.opacity = 0.5;

        let (vertices, _) = build_quads(&scene);

        assert_eq!(vertices[0].color[3], 0.25);
    }

    #[test]
    fn color_channels_are_converted_from_bytes_to_unit_floats() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "div", (0.0, 0.0, 10.0, 10.0));
        scene.get_mut(root).style.background_color = Color::from_rgba(255, 128, 0, 1.0);

        let (vertices, _) = build_quads(&scene);

        assert_eq!(vertices[0].color[0], 1.0);
        assert!((vertices[0].color[1] - 128.0 / 255.0).abs() < 1e-6);
        assert_eq!(vertices[0].color[2], 0.0);
    }

    #[test]
    fn per_corner_border_radii_are_carried_on_every_vertex() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "div", (0.0, 0.0, 40.0, 40.0));
        opaque(&mut scene, root, 255, 0, 0);
        // top-left, top-right, bottom-right, bottom-left
        scene.get_mut(root).style.border_radius = [1.0, 2.0, 3.0, 4.0];

        let (vertices, _) = build_quads(&scene);

        assert!(vertices
            .iter()
            .all(|v| v.border_radius == [1.0, 2.0, 3.0, 4.0]));
    }

    #[test]
    fn a_scroll_offset_shifts_descendants_but_not_the_container() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "div", (0.0, 0.0, 100.0, 100.0));
        let child = node(&mut scene, "div", (0.0, 60.0, 100.0, 20.0));
        scene.add_child(root, child);
        opaque(&mut scene, root, 255, 0, 0);
        opaque(&mut scene, child, 0, 255, 0);
        scene.get_mut(root).scroll_offset = (0.0, 25.0);

        let (vertices, _) = build_quads(&scene);

        assert_eq!(vertices[0].rect, [0.0, 0.0, 100.0, 100.0]);
        assert_eq!(vertices[4].rect, [0.0, 35.0, 100.0, 20.0]);
    }

    #[test]
    fn the_vertex_layout_stride_matches_the_pipeline_attributes() {
        // position(8) + color(16) + border_color(16) + rect(16) + radius(16)
        // + border_width(4) + padding(12) == 88 bytes, 4-byte aligned for wgpu
        assert_eq!(std::mem::size_of::<QuadVertex>(), 88);
        assert_eq!(std::mem::align_of::<QuadVertex>(), 4);
    }

    // ---- clear colour ----

    #[test]
    fn the_clear_colour_comes_from_the_root_background() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "html", (0.0, 0.0, 100.0, 100.0));
        opaque(&mut scene, root, 255, 0, 0);

        let c = clear_color(&scene);

        assert_eq!((c.r, c.g, c.b, c.a), (1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn the_clear_colour_falls_back_to_the_body_background() {
        let mut scene = SceneGraph::new();
        let root = node(&mut scene, "html", (0.0, 0.0, 100.0, 100.0));
        let body = node(&mut scene, "body", (0.0, 0.0, 100.0, 100.0));
        scene.add_child(root, body);
        opaque(&mut scene, body, 0, 0, 255);

        let c = clear_color(&scene);

        assert_eq!((c.r, c.g, c.b, c.a), (0.0, 0.0, 1.0, 1.0));
    }

    #[test]
    fn the_clear_colour_defaults_to_white() {
        let mut scene = SceneGraph::new();
        node(&mut scene, "html", (0.0, 0.0, 100.0, 100.0));

        assert_eq!(clear_color(&scene).r, 1.0);
        assert_eq!(clear_color(&SceneGraph::new()).r, 1.0);
    }

    // ---- real GPU: the SDF quad pipeline ----

    const TEX: u32 = 64;

    /// Render `scene` into a 64x64 RGBA8 texture with the real quad pipeline
    /// and return the raw pixels.
    fn render_to_pixels(device: &wgpu::Device, queue: &wgpu::Queue, scene: &SceneGraph) -> Vec<u8> {
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let layout = create_viewport_bind_group_layout(device);
        let pipeline = create_quad_pipeline(device, &layout, format);

        let viewport: [f32; 4] = [TEX as f32, TEX as f32, 0.0, 0.0];
        let viewport_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&viewport),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: viewport_buffer.as_entire_binding(),
            }],
        });

        let (vertices, indices) = build_quads(scene);
        assert!(!indices.is_empty(), "test scene draws nothing");
        let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: TEX,
                height: TEX,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());

        // 64 px * 4 bytes == 256, already row-alignment friendly
        let bytes = (TEX * TEX * 4) as u64;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // transparent, so uncovered pixels are unambiguous
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_vertex_buffer(0, vbuf.slice(..));
            pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(TEX * 4),
                    rows_per_image: Some(TEX),
                },
            },
            wgpu::Extent3d {
                width: TEX,
                height: TEX,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(Some(encoder.finish()));

        readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll failed");
        let data = readback.slice(..).get_mapped_range().to_vec();
        readback.unmap();
        data
    }

    fn pixel(data: &[u8], x: u32, y: u32) -> [u8; 4] {
        let i = ((y * TEX + x) * 4) as usize;
        [data[i], data[i + 1], data[i + 2], data[i + 3]]
    }

    #[test]
    fn the_sdf_shader_fills_a_rounded_rect_and_cuts_its_corners() {
        with_headless_device(
            "the_sdf_shader_fills_a_rounded_rect_and_cuts_its_corners",
            |device, queue| {
                let mut scene = SceneGraph::new();
                let root = node(&mut scene, "div", (8.0, 8.0, 48.0, 48.0));
                opaque(&mut scene, root, 255, 0, 0);
                scene.get_mut(root).style.border_radius = [16.0; 4];

                let data = render_to_pixels(device, queue, &scene);

                // dead centre is solid fill
                assert_eq!(pixel(&data, 32, 32), [255, 0, 0, 255]);
                // middle of the top edge is inside the shape
                assert_eq!(pixel(&data, 32, 9), [255, 0, 0, 255]);
                // the rounded corner is cut away: 20.5px from the corner centre
                // with a 16px radius is outside the antialiasing band
                assert_eq!(pixel(&data, 9, 9)[3], 0, "top-left corner not rounded");
                assert_eq!(
                    pixel(&data, 54, 54)[3],
                    0,
                    "bottom-right corner not rounded"
                );
                // outside the rect entirely
                assert_eq!(pixel(&data, 2, 32)[3], 0);
            },
        );
    }

    #[test]
    fn a_square_quad_keeps_its_corners() {
        with_headless_device("a_square_quad_keeps_its_corners", |device, queue| {
            let mut scene = SceneGraph::new();
            let root = node(&mut scene, "div", (8.0, 8.0, 48.0, 48.0));
            opaque(&mut scene, root, 0, 255, 0);

            let data = render_to_pixels(device, queue, &scene);

            assert_eq!(pixel(&data, 32, 32), [0, 255, 0, 255]);
            assert_eq!(pixel(&data, 9, 9), [0, 255, 0, 255]);
            assert_eq!(pixel(&data, 54, 54), [0, 255, 0, 255]);
        });
    }

    #[test]
    fn the_sdf_shader_paints_the_border_ring_in_the_border_colour() {
        with_headless_device(
            "the_sdf_shader_paints_the_border_ring_in_the_border_colour",
            |device, queue| {
                let mut scene = SceneGraph::new();
                let root = node(&mut scene, "div", (8.0, 8.0, 48.0, 48.0));
                opaque(&mut scene, root, 255, 0, 0);
                scene.get_mut(root).style.border_width = [6.0; 4];
                scene.get_mut(root).style.border_color = Color::from_rgba(0, 0, 255, 1.0);

                let data = render_to_pixels(device, queue, &scene);

                // 2px inside the top edge is border, the centre is fill
                assert_eq!(pixel(&data, 32, 10), [0, 0, 255, 255]);
                assert_eq!(pixel(&data, 10, 32), [0, 0, 255, 255]);
                assert_eq!(pixel(&data, 32, 32), [255, 0, 0, 255]);
            },
        );
    }

    #[test]
    fn the_shader_maps_pixel_space_to_clip_space_with_y_pointing_down() {
        with_headless_device(
            "the_shader_maps_pixel_space_to_clip_space_with_y_pointing_down",
            |device, queue| {
                let mut scene = SceneGraph::new();
                // a bar across the top quarter of the surface
                let root = node(&mut scene, "div", (0.0, 0.0, 64.0, 16.0));
                opaque(&mut scene, root, 0, 0, 255);

                let data = render_to_pixels(device, queue, &scene);

                assert_eq!(
                    pixel(&data, 32, 4),
                    [0, 0, 255, 255],
                    "top should be painted"
                );
                assert_eq!(pixel(&data, 32, 60)[3], 0, "bottom should be untouched");
            },
        );
    }
}
