//! The video picture: decoded frames go into one GPU texture, which is
//! updated in place and drawn letterboxed. Iced's image widget would
//! create (and later evict) a new texture for every frame.
use std::fmt;
use std::sync::Arc;

use iced::mouse;
use iced::wgpu;
use iced::widget::shader::{self, Viewport};
use iced::Rectangle;

/// A decoded frame, RGBA, shared with the GPU upload without copying.
pub struct Frame {
    /// Grows by one for each new frame; the texture only changes when it does.
    pub serial: u64,
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// The shader widget's program: draws the newest frame.
pub struct Video(pub Arc<Frame>);

impl<M> shader::Program<M> for Video {
    type State = ();
    type Primitive = Primitive;

    fn draw(&self, _: &(), _: mouse::Cursor, _: Rectangle) -> Primitive {
        Primitive(self.0.clone())
    }
}

pub struct Primitive(Arc<Frame>);

impl fmt::Debug for Primitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "video frame {} ({}×{})",
            self.0.serial, self.0.w, self.0.h
        )
    }
}

pub struct Pipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Where the picture sits in the widget: x and y scale (letterboxing).
    uniform: wgpu::Buffer,
    /// The surface stores sRGB: sample the bytes as sRGB too, so they show as decoded.
    srgb: bool,
    texture: Option<Texture>,
}

struct Texture {
    texture: wgpu::Texture,
    bind: wgpu::BindGroup,
    w: u32,
    h: u32,
    serial: u64,
}

const SHADER: &str = r"
struct U { scale: vec2<f32>, pad: vec2<f32> };
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> u: U;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Out {
    var corners = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, 1.0),
    );
    let p = corners[i];
    var o: Out;
    o.pos = vec4<f32>(p * u.scale, 0.0, 1.0);
    o.uv = vec2<f32>((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5);
    return o;
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, i.uv);
}
";

impl shader::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("valo video"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("valo video"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("valo video"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("valo video"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("valo video"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("valo video"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            layout,
            sampler,
            uniform,
            srgb: format.is_srgb(),
            texture: None,
        }
    }
}

impl shader::Primitive for Primitive {
    type Pipeline = Pipeline;

    fn prepare(
        &self,
        p: &mut Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        _viewport: &Viewport,
    ) {
        let f = &self.0;
        if f.w == 0 || f.h == 0 || f.rgba.len() < (f.w * f.h * 4) as usize {
            return;
        }
        if p.texture.as_ref().is_none_or(|t| t.w != f.w || t.h != f.h) {
            let format = if p.srgb {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            };
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("valo video"),
                size: wgpu::Extent3d {
                    width: f.w,
                    height: f.h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("valo video"),
                layout: &p.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&p.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: p.uniform.as_entire_binding(),
                    },
                ],
            });
            p.texture = Some(Texture {
                texture,
                bind,
                w: f.w,
                h: f.h,
                serial: u64::MAX,
            });
        }
        let Some(t) = p.texture.as_mut() else { return };
        if t.serial != f.serial {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &t.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &f.rgba[..(f.w * f.h * 4) as usize],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(f.w * 4),
                    rows_per_image: Some(f.h),
                },
                wgpu::Extent3d {
                    width: f.w,
                    height: f.h,
                    depth_or_array_layers: 1,
                },
            );
            t.serial = f.serial;
        }
        // fit the picture in the widget, keeping its shape
        let (bw, bh) = (bounds.width.max(1.0), bounds.height.max(1.0));
        let (fa, ba) = (f.w as f32 / f.h as f32, bw / bh);
        let scale = if ba > fa {
            [fa / ba, 1.0]
        } else {
            [1.0, ba / fa]
        };
        let mut bytes = [0u8; 16];
        bytes[..4].copy_from_slice(&scale[0].to_le_bytes());
        bytes[4..8].copy_from_slice(&scale[1].to_le_bytes());
        queue.write_buffer(&p.uniform, 0, &bytes);
    }

    fn draw(&self, p: &Pipeline, pass: &mut wgpu::RenderPass<'_>) -> bool {
        let Some(t) = &p.texture else { return true };
        pass.set_pipeline(&p.pipeline);
        pass.set_bind_group(0, &t.bind, &[]);
        pass.draw(0..4, 0..1);
        true
    }
}
