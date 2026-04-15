//! GPU text renderer. A 3×5 bitmap font; one fragment = one buffer fetch +
//! one bit extract. Glyphs are defined visually in Rust (see [`bitmap`])
//! and uploaded to a storage buffer at construction.

use bytemuck::{Pod, Zeroable};

const TEXT_WGSL: &str = include_str!("shaders/text.wgsl");

/// Font covers ASCII codes 32..=126 (95 printable codes). Earlier codes are
/// zero-padded so indexing by ASCII value works directly.
const GLYPH_TABLE_SIZE: usize = 128;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct TextParams {
    origin_px: [f32; 2],
    scale: f32,
    _pad0: f32,
    color: [f32; 4],
    screen_size: [f32; 2],
    text_len: u32,
    _pad1: u32,
}

pub struct TextRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    params_buffer: wgpu::Buffer,
    text_buffer: wgpu::Buffer,
    max_chars: u32,
    params: TextParams,
}

impl TextRenderer {
    /// `target_format` must match the texture the `render` call writes to.
    /// `max_chars` caps the string length (fixed at construction).
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        max_chars: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gpu_util::text shader"),
            source: wgpu::ShaderSource::Wgsl(TEXT_WGSL.into()),
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_util::text params"),
            size: std::mem::size_of::<TextParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let text_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_util::text content"),
            size: (max_chars as u64) * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let glyph_table = build_glyph_table();
        let glyph_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_util::text glyphs"),
            size: (GLYPH_TABLE_SIZE as u64) * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&glyph_buffer, 0, bytemuck::cast_slice(&glyph_table));

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpu_util::text bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_util::text bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: text_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: glyph_buffer.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpu_util::text layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gpu_util::text pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group,
            params_buffer,
            text_buffer,
            max_chars,
            params: TextParams {
                origin_px: [0.0, 0.0],
                scale: 2.0,
                _pad0: 0.0,
                color: [1.0, 1.0, 1.0, 1.0],
                screen_size: [1920.0, 1080.0],
                text_len: 0,
                _pad1: 0,
            },
        }
    }

    /// Replace the current text. ASCII-only; non-ASCII is mapped to '?'.
    /// Truncated if longer than `max_chars`.
    pub fn set_text(&mut self, queue: &wgpu::Queue, text: &str) {
        let mut codes: Vec<u32> = Vec::with_capacity(self.max_chars as usize);
        for ch in text.chars() {
            if codes.len() as u32 >= self.max_chars {
                break;
            }
            let c = if ch.is_ascii() { ch as u32 } else { b'?' as u32 };
            codes.push(c);
        }
        self.params.text_len = codes.len() as u32;
        while codes.len() < self.max_chars as usize {
            codes.push(0);
        }
        queue.write_buffer(&self.text_buffer, 0, bytemuck::cast_slice(&codes));
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&self.params));
    }

    /// Position, scale, color. Call this when the window resizes or you want
    /// to move the text. `origin_px` is screen-pixel top-left.
    pub fn set_params(
        &mut self,
        queue: &wgpu::Queue,
        origin_px: [f32; 2],
        scale: f32,
        color: [f32; 4],
        screen_size: [f32; 2],
    ) {
        self.params.origin_px = origin_px;
        self.params.scale = scale;
        self.params.color = color;
        self.params.screen_size = screen_size;
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&self.params));
    }

    /// Alpha-blend the text onto `target`.
    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("gpu_util::text pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..4, 0..1);
    }
}

/// Pack a 3×5 visual glyph (top-to-bottom rows) into a u32. Row 0 occupies
/// bits 0..=2, row 4 occupies bits 12..=14. `X` = pixel set, anything else = blank.
fn bitmap(rows: [&str; 5]) -> u32 {
    let mut v = 0u32;
    for (r, row) in rows.iter().enumerate() {
        for (c, ch) in row.chars().enumerate().take(3) {
            if ch == 'X' {
                v |= 1 << (r * 3 + c);
            }
        }
    }
    v
}

fn build_glyph_table() -> [u32; GLYPH_TABLE_SIZE] {
    let mut g = [0u32; GLYPH_TABLE_SIZE];

    // 32 = space
    g[b' ' as usize] = 0;

    g[b'!' as usize] = bitmap([
        ".X.",
        ".X.",
        ".X.",
        "...",
        ".X.",
    ]);

    g[b'.' as usize] = bitmap([
        "...",
        "...",
        "...",
        "...",
        ".X.",
    ]);

    g[b',' as usize] = bitmap([
        "...",
        "...",
        "...",
        ".X.",
        ".X.",
    ]);

    g[b':' as usize] = bitmap([
        ".X.",
        "...",
        "...",
        "...",
        ".X.",
    ]);

    g[b';' as usize] = bitmap([
        ".X.",
        "...",
        "...",
        ".X.",
        ".X.",
    ]);

    g[b'-' as usize] = bitmap([
        "...",
        "...",
        "XXX",
        "...",
        "...",
    ]);

    g[b'+' as usize] = bitmap([
        "...",
        ".X.",
        "XXX",
        ".X.",
        "...",
    ]);

    g[b'=' as usize] = bitmap([
        "...",
        "XXX",
        "...",
        "XXX",
        "...",
    ]);

    g[b'/' as usize] = bitmap([
        "..X",
        "..X",
        ".X.",
        "X..",
        "X..",
    ]);

    g[b'\\' as usize] = bitmap([
        "X..",
        "X..",
        ".X.",
        "..X",
        "..X",
    ]);

    g[b'|' as usize] = bitmap([
        ".X.",
        ".X.",
        ".X.",
        ".X.",
        ".X.",
    ]);

    g[b'(' as usize] = bitmap([
        "..X",
        ".X.",
        ".X.",
        ".X.",
        "..X",
    ]);

    g[b')' as usize] = bitmap([
        "X..",
        ".X.",
        ".X.",
        ".X.",
        "X..",
    ]);

    g[b'[' as usize] = bitmap([
        ".XX",
        ".X.",
        ".X.",
        ".X.",
        ".XX",
    ]);

    g[b']' as usize] = bitmap([
        "XX.",
        ".X.",
        ".X.",
        ".X.",
        "XX.",
    ]);

    g[b'_' as usize] = bitmap([
        "...",
        "...",
        "...",
        "...",
        "XXX",
    ]);

    g[b'#' as usize] = bitmap([
        "X.X",
        "XXX",
        "X.X",
        "XXX",
        "X.X",
    ]);

    g[b'*' as usize] = bitmap([
        "...",
        "X.X",
        ".X.",
        "X.X",
        "...",
    ]);

    g[b'%' as usize] = bitmap([
        "X.X",
        "..X",
        ".X.",
        "X..",
        "X.X",
    ]);

    g[b'?' as usize] = bitmap([
        ".X.",
        "...",
        ".X.",
        "..X",
        "XX.",
    ]);

    // Digits 0-9
    g[b'0' as usize] = bitmap([
        "XXX",
        "X.X",
        "X.X",
        "X.X",
        "XXX",
    ]);
    g[b'1' as usize] = bitmap([
        "XXX",
        ".X.",
        ".X.",
        "XX.",
        ".X.",
    ]);
    g[b'2' as usize] = bitmap([
        "XXX",
        "X..",
        "XXX",
        "..X",
        "XXX",
    ]);
    g[b'3' as usize] = bitmap([
        "XXX",
        "..X",
        ".XX",
        "..X",
        "XXX",
    ]);
    g[b'4' as usize] = bitmap([
        "..X",
        "..X",
        "XXX",
        "X.X",
        "X.X",
    ]);
    g[b'5' as usize] = bitmap([
        "XXX",
        "..X",
        "XXX",
        "X..",
        "XXX",
    ]);
    g[b'6' as usize] = bitmap([
        "XXX",
        "X.X",
        "XXX",
        "X..",
        "XXX",
    ]);
    g[b'7' as usize] = bitmap([
        "..X",
        "..X",
        "..X",
        "..X",
        "XXX",
    ]);
    g[b'8' as usize] = bitmap([
        "XXX",
        "X.X",
        "XXX",
        "X.X",
        "XXX",
    ]);
    g[b'9' as usize] = bitmap([
        "XXX",
        "..X",
        "XXX",
        "X.X",
        "XXX",
    ]);

    // Uppercase A-Z
    g[b'A' as usize] = bitmap([
        "X.X",
        "X.X",
        "XXX",
        "X.X",
        ".X.",
    ]);
    g[b'B' as usize] = bitmap([
        "XX.",
        "X.X",
        "XX.",
        "X.X",
        "XX.",
    ]);
    g[b'C' as usize] = bitmap([
        "XXX",
        "X..",
        "X..",
        "X..",
        "XXX",
    ]);
    g[b'D' as usize] = bitmap([
        "XX.",
        "X.X",
        "X.X",
        "X.X",
        "XX.",
    ]);
    g[b'E' as usize] = bitmap([
        "XXX",
        "X..",
        "XX.",
        "X..",
        "XXX",
    ]);
    g[b'F' as usize] = bitmap([
        "X..",
        "X..",
        "XX.",
        "X..",
        "XXX",
    ]);
    g[b'G' as usize] = bitmap([
        "XXX",
        "X.X",
        "X..",
        "X..",
        "XXX",
    ]);
    g[b'H' as usize] = bitmap([
        "X.X",
        "X.X",
        "XXX",
        "X.X",
        "X.X",
    ]);
    g[b'I' as usize] = bitmap([
        "XXX",
        ".X.",
        ".X.",
        ".X.",
        "XXX",
    ]);
    g[b'J' as usize] = bitmap([
        "XX.",
        "X.X",
        "..X",
        "..X",
        "XXX",
    ]);
    g[b'K' as usize] = bitmap([
        "X.X",
        "XX.",
        "X..",
        "XX.",
        "X.X",
    ]);
    g[b'L' as usize] = bitmap([
        "XXX",
        "X..",
        "X..",
        "X..",
        "X..",
    ]);
    g[b'M' as usize] = bitmap([
        "X.X",
        "X.X",
        "X.X",
        "XXX",
        "X.X",
    ]);
    g[b'N' as usize] = bitmap([
        "X.X",
        "X.X",
        "XXX",
        "XXX",
        "X.X",
    ]);
    g[b'O' as usize] = bitmap([
        "XXX",
        "X.X",
        "X.X",
        "X.X",
        "XXX",
    ]);
    g[b'P' as usize] = bitmap([
        "X..",
        "X..",
        "XXX",
        "X.X",
        "XXX",
    ]);
    g[b'Q' as usize] = bitmap([
        ".XX",
        "XX.",
        "X.X",
        "X.X",
        "XXX",
    ]);
    g[b'R' as usize] = bitmap([
        "X.X",
        "XX.",
        "XXX",
        "X.X",
        "XXX",
    ]);
    g[b'S' as usize] = bitmap([
        "XXX",
        "..X",
        "XXX",
        "X..",
        "XXX",
    ]);
    g[b'T' as usize] = bitmap([
        ".X.",
        ".X.",
        ".X.",
        ".X.",
        "XXX",
    ]);
    g[b'U' as usize] = bitmap([
        "XXX",
        "X.X",
        "X.X",
        "X.X",
        "X.X",
    ]);
    g[b'V' as usize] = bitmap([
        ".X.",
        "X.X",
        "X.X",
        "X.X",
        "X.X",
    ]);
    g[b'W' as usize] = bitmap([
        "X.X",
        "XXX",
        "X.X",
        "X.X",
        "X.X",
    ]);
    g[b'X' as usize] = bitmap([
        "X.X",
        "X.X",
        ".X.",
        "X.X",
        "X.X",
    ]);
    g[b'Y' as usize] = bitmap([
        ".X.",
        ".X.",
        ".X.",
        "X.X",
        "X.X",
    ]);
    g[b'Z' as usize] = bitmap([
        "XXX",
        "X..",
        ".X.",
        "..X",
        "XXX",
    ]);

    // Lowercase: alias to uppercase (compact, readable, saves time).
    for c in b'a'..=b'z' {
        g[c as usize] = g[(c - 32) as usize];
    }

    g
}
