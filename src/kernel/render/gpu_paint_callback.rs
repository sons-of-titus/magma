//! egui wgpu `PaintCallback` for batched glyph and background rendering.
//!
//! `MagmaPaintCallback` is registered via `egui_wgpu::Callback::new_paint_callback`
//! once per frame.  On the first frame it creates all wgpu resources (pipeline,
//! atlas texture, vertex buffer) and stores them in `egui_wgpu::CallbackResources`
//! so they survive frame-to-frame.  Subsequent frames only update the vertex
//! buffer, and re-upload the atlas texture when `atlas_pixels` is `Some`.

use eframe::egui_wgpu::{self, ScreenDescriptor};
use eframe::wgpu::{self, util::DeviceExt as _};

use super::gpu_atlas::{GlyphInstance, RectInstance, glyphs_to_vertices, rects_to_vertices};

// ── WGSL shader ───────────────────────────────────────────────────────────────

const SHADER_SRC: &str = r#"
struct Screen { width: f32, height: f32, _p0: f32, _p1: f32 }
@group(0) @binding(0) var<uniform> screen: Screen;
@group(1) @binding(0) var atlas_tex: texture_2d<f32>;
@group(1) @binding(1) var atlas_smp: sampler;

struct VIn  { @location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32> }
struct VOut { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) color: vec4<f32> }

@vertex fn vs(in: VIn) -> VOut {
    var o: VOut;
    o.clip  = vec4<f32>(in.pos.x * 2.0 / screen.width - 1.0,
                        1.0 - in.pos.y * 2.0 / screen.height, 0.0, 1.0);
    o.uv    = in.uv;
    o.color = in.color;
    return o;
}

@fragment fn fs(in: VOut) -> @location(0) vec4<f32> {
    return textureSample(atlas_tex, atlas_smp, in.uv) * in.color;
}
"#;

// ── GPU resources (stored in CallbackResources across frames) ─────────────────

pub struct GpuRenderResources {
    pipeline:        wgpu::RenderPipeline,
    vertex_buf:      wgpu::Buffer,
    screen_uniform:  wgpu::Buffer,
    screen_bind:     wgpu::BindGroup,
    atlas_bind:      wgpu::BindGroup,
    /// Kept so we can rebuild `atlas_bind` when the font changes.
    atlas_bgl:       wgpu::BindGroupLayout,
    vertex_capacity: u64,
    vertex_count:    u32,
}

const VERTEX_STRIDE: u64 = std::mem::size_of::<[f32; 8]>() as u64;
const INITIAL_VERTS:  u64 = 4096;

impl GpuRenderResources {
    pub fn create(
        device:        &wgpu::Device,
        queue:         &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        atlas_width:   u32,
        atlas_height:  u32,
        atlas_pixels:  &[u8],
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("magma_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        // group 0: screen uniform
        let screen_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:   Some("magma_screen_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding:    0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty:         wgpu::BindingType::Buffer {
                    ty:                 wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size:   None,
                },
                count: None,
            }],
        });

        // group 1: atlas texture + sampler
        let atlas_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:   Some("magma_atlas_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding:    0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty:         wgpu::BindingType::Texture {
                        sample_type:    wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled:   false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding:    1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty:         wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label:                Some("magma_pip_layout"),
            bind_group_layouts:   &[&screen_bgl, &atlas_bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("magma_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module:              &shader,
                entry_point:         "vs",
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: VERTEX_STRIDE,
                    step_mode:    wgpu::VertexStepMode::Vertex,
                    attributes:   &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x2,
                        2 => Float32x4,
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module:              &shader,
                entry_point:         "fs",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format:     target_format,
                    blend:      Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive:     wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample:   wgpu::MultisampleState::default(),
            multiview:     None,
            cache:         None,
        });

        let atlas_bind = make_atlas_bind(device, queue, &atlas_bgl, atlas_width, atlas_height, atlas_pixels);

        // Screen uniform buffer
        let screen_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some("magma_screen_uniform"),
            contents: bytemuck::cast_slice(&[0.0f32; 4]),
            usage:    wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let screen_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("magma_screen_bg"),
            layout:  &screen_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: screen_uniform.as_entire_binding() }],
        });

        // Vertex buffer (preallocated)
        let vertex_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("magma_vbuf"),
            size:               INITIAL_VERTS * VERTEX_STRIDE,
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        GpuRenderResources {
            pipeline, vertex_buf, screen_uniform, screen_bind,
            atlas_bind, atlas_bgl,
            vertex_capacity: INITIAL_VERTS, vertex_count: 0,
        }
    }

    /// Re-upload the atlas texture (called when the font changes).
    pub fn update_atlas(
        &mut self,
        device: &wgpu::Device,
        queue:  &wgpu::Queue,
        width:  u32,
        height: u32,
        pixels: &[u8],
    ) {
        self.atlas_bind = make_atlas_bind(device, queue, &self.atlas_bgl, width, height, pixels);
    }

    fn update_vertices(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, verts: &[[f32; 8]]) {
        let needed = verts.len() as u64;
        if needed > self.vertex_capacity {
            self.vertex_capacity = needed.next_power_of_two();
            self.vertex_buf = device.create_buffer(&wgpu::BufferDescriptor {
                label:              Some("magma_vbuf"),
                size:               self.vertex_capacity * VERTEX_STRIDE,
                usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !verts.is_empty() {
            queue.write_buffer(&self.vertex_buf, 0, bytemuck::cast_slice(verts));
        }
        self.vertex_count = verts.len() as u32;
    }
}

/// Create (or recreate) the atlas texture and its bind group.
fn make_atlas_bind(
    device:  &wgpu::Device,
    queue:   &wgpu::Queue,
    bgl:     &wgpu::BindGroupLayout,
    width:   u32,
    height:  u32,
    pixels:  &[u8],
) -> wgpu::BindGroup {
    let tex_size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label:           Some("magma_atlas"),
        size:            tex_size,
        mip_level_count: 1,
        sample_count:    1,
        dimension:       wgpu::TextureDimension::D2,
        format:          wgpu::TextureFormat::Rgba8Unorm,
        usage:           wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats:    &[],
    });
    queue.write_texture(
        tex.as_image_copy(),
        pixels,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: None,
        },
        tex_size,
    );
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label:      Some("magma_sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label:   Some("magma_atlas_bg"),
        layout:  bgl,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
        ],
    })
}

// ── PaintCallback ─────────────────────────────────────────────────────────────

/// Submitted to egui's painter once per frame to batch all glyph and rect draws.
pub struct MagmaPaintCallback {
    pub glyphs:        Vec<GlyphInstance>,
    pub rects:         Vec<RectInstance>,
    pub char_w:        f32,
    pub line_h:        f32,
    pub target_format: wgpu::TextureFormat,
    pub atlas_width:   u32,
    pub atlas_height:  u32,
    /// `Some(pixels)` → re-upload atlas texture this frame; `None` → reuse.
    pub atlas_pixels:  Option<Vec<u8>>,
}

impl MagmaPaintCallback {
    pub fn into_paint_callback(self, rect: eframe::egui::Rect) -> eframe::epaint::PaintCallback {
        egui_wgpu::Callback::new_paint_callback(rect, self)
    }
}

impl egui_wgpu::CallbackTrait for MagmaPaintCallback {
    fn prepare(
        &self,
        device:            &wgpu::Device,
        queue:             &wgpu::Queue,
        screen_descriptor: &ScreenDescriptor,
        _egui_encoder:     &mut wgpu::CommandEncoder,
        resources:         &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let first_frame = resources.get::<GpuRenderResources>().is_none();

        if first_frame {
            // Use provided pixels (rasterized) or fall back to placeholder.
            let placeholder;
            let pixels: &[u8] = match &self.atlas_pixels {
                Some(p) => p,
                None => {
                    placeholder = vec![255u8; (self.atlas_width * self.atlas_height * 4) as usize];
                    &placeholder
                }
            };
            let res = GpuRenderResources::create(
                device, queue, self.target_format,
                self.atlas_width, self.atlas_height, pixels,
            );
            resources.insert(res);
        } else if let Some(pixels) = &self.atlas_pixels {
            // Font changed — re-upload atlas texture.
            let res = resources.get_mut::<GpuRenderResources>().unwrap();
            res.update_atlas(device, queue, self.atlas_width, self.atlas_height, pixels);
        }

        let res = resources.get_mut::<GpuRenderResources>().unwrap();

        // Update screen uniform
        let [w, h] = screen_descriptor.size_in_pixels;
        let ppx = screen_descriptor.pixels_per_point;
        queue.write_buffer(
            &res.screen_uniform, 0,
            bytemuck::cast_slice(&[w as f32 / ppx, h as f32 / ppx, 0.0f32, 0.0f32]),
        );

        // Build vertex buffer: rects first (background), then glyphs (foreground).
        let mut verts = rects_to_vertices(&self.rects, self.atlas_width, self.atlas_height);
        verts.extend(glyphs_to_vertices(&self.glyphs, self.char_w, self.line_h));
        res.update_vertices(device, queue, &verts);

        vec![]
    }

    fn paint(
        &self,
        _info:       eframe::epaint::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources:   &egui_wgpu::CallbackResources,
    ) {
        let Some(res) = resources.get::<GpuRenderResources>() else { return };
        if res.vertex_count == 0 { return }
        render_pass.set_pipeline(&res.pipeline);
        render_pass.set_bind_group(0, &res.screen_bind, &[]);
        render_pass.set_bind_group(1, &res.atlas_bind, &[]);
        render_pass.set_vertex_buffer(0, res.vertex_buf.slice(..));
        render_pass.draw(0..res.vertex_count, 0..1);
    }
}
