use rustshade_core::EffectSpec;
use rustshade_fx::Lut3D;
use std::collections::HashMap;
use wgpu::util::DeviceExt;

#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    #[error("no compatible GPU adapter found (CPU fallback is available via --no-gpu)")]
    NoAdapter,
    #[error("device request failed: {0}")]
    Request(#[from] wgpu::RequestDeviceError),
    #[error("GPU operation failed: {0}")]
    Operation(String),
    #[error("lut error: {0}")]
    Lut(#[from] rustshade_fx::effects::lut::LutError),
    #[error("uniform exceeds 64 bytes")]
    UniformOverflow,
}

const COMMON_WGSL: &str = include_str!("shaders/common.wgsl");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum LayoutKind {
    Standard,
    Lut,
    BloomComposite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PipelineId {
    Grade,
    Vignette,
    ChromaticAberration,
    Sharpen,
    Fxaa,
    Grain,
    Scanlines,
    Lut,
    BloomThreshold,
    BloomBlur,
    BloomComposite,
}

impl PipelineId {
    fn from_effect_id(id: &str) -> Option<Self> {
        Some(match id {
            "grade" => Self::Grade,
            "vignette" => Self::Vignette,
            "chromatic_aberration" => Self::ChromaticAberration,
            "sharpen" => Self::Sharpen,
            "fxaa" => Self::Fxaa,
            "grain" => Self::Grain,
            "scanlines" => Self::Scanlines,
            "lut" => Self::Lut,
            _ => return None,
        })
    }

    fn source(self) -> &'static str {
        match self {
            Self::Grade => include_str!("shaders/grade.wgsl"),
            Self::Vignette => include_str!("shaders/vignette.wgsl"),
            Self::ChromaticAberration => include_str!("shaders/chromatic_aberration.wgsl"),
            Self::Sharpen => include_str!("shaders/sharpen.wgsl"),
            Self::Fxaa => include_str!("shaders/fxaa.wgsl"),
            Self::Grain => include_str!("shaders/grain.wgsl"),
            Self::Scanlines => include_str!("shaders/scanlines.wgsl"),
            Self::Lut => include_str!("shaders/lut.wgsl"),
            Self::BloomThreshold => include_str!("shaders/bloom_threshold.wgsl"),
            Self::BloomBlur => include_str!("shaders/bloom_blur.wgsl"),
            Self::BloomComposite => include_str!("shaders/bloom_composite.wgsl"),
        }
    }

    fn layout(self) -> LayoutKind {
        match self {
            Self::Lut => LayoutKind::Lut,
            Self::BloomComposite => LayoutKind::BloomComposite,
            _ => LayoutKind::Standard,
        }
    }
}

const UNIFORM_SIZE: usize = 64;

#[derive(Debug, Clone, Copy, Default)]
struct UniformBlock {
    p0: [f32; 4],
    p1: [f32; 4],
    p2: [f32; 4],
    dims: [f32; 2],
    frame: f32,
    pad: f32,
}

impl UniformBlock {
    fn bytes(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(UNIFORM_SIZE);
        for v in self
            .p0
            .iter()
            .chain(self.p1.iter())
            .chain(self.p2.iter())
            .chain(self.dims.iter())
            .chain([self.frame, self.pad].iter())
        {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }
}

fn get(spec: &EffectSpec, name: &str, default: f32) -> f32 {
    spec.get_f32(name).unwrap_or(default)
}

/// Packs effect params into the fixed uniform layout (see shader headers).
fn uniform_for(
    spec: &EffectSpec,
    w: u32,
    h: u32,
    frame_index: u64,
    extra: [f32; 4],
) -> UniformBlock {
    let p0: [f32; 4] = match spec.id.as_str() {
        "grade" => [
            get(spec, "exposure", 0.0),
            get(spec, "contrast", 0.0),
            get(spec, "saturation", 1.0),
            get(spec, "temperature", 0.0),
        ],
        "bloom" => [
            get(spec, "threshold", 0.75),
            get(spec, "knee", 0.3),
            0.0,
            0.0,
        ],
        "sharpen" => [
            get(spec, "amount", 0.5),
            get(spec, "radius", 1.0),
            get(spec, "threshold", 0.0),
            0.0,
        ],
        "fxaa" => [
            get(spec, "subpixel", 0.75),
            get(spec, "edge_threshold", 0.125),
            0.0,
            0.0,
        ],
        "vignette" => [
            get(spec, "amount", 0.5),
            get(spec, "inner", 0.35),
            get(spec, "softness", 0.6),
            0.0,
        ],
        "chromatic_aberration" => [get(spec, "strength", 0.2), 0.0, 0.0, 0.0],
        "grain" => [
            get(spec, "strength", 0.1),
            get(spec, "seed", 0.0),
            get(spec, "animate", 1.0),
            0.0,
        ],
        "scanlines" => [
            get(spec, "amount", 0.2),
            get(spec, "period", 3.0),
            get(spec, "phase", 0.0),
            0.0,
        ],
        "lut" => [get(spec, "opacity", 1.0), 0.0, 0.0, 0.0],
        _ => [0.0; 4],
    };
    // Extra values carry per-pipeline data (blur direction, lut size).
    let p1: [f32; 4] = extra;
    UniformBlock {
        p0,
        p1,
        p2: [0.0; 4],
        dims: [w as f32, h as f32],
        frame: frame_index as f32,
        pad: 0.0,
    }
}

pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipelines: HashMap<PipelineId, wgpu::RenderPipeline>,
    bgl_render: wgpu::BindGroupLayout,
    bgl_lut: wgpu::BindGroupLayout,
    bgl_composite: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    tex_a: Option<(wgpu::Texture, wgpu::TextureView)>,
    tex_b: Option<(wgpu::Texture, wgpu::TextureView)>,
    small_a: Option<(wgpu::Texture, wgpu::TextureView)>,
    small_b: Option<(wgpu::Texture, wgpu::TextureView)>,
    luts: HashMap<String, (u32, wgpu::Texture, wgpu::TextureView)>,
    lut_base: std::path::PathBuf,
}

fn resolve_lut_path(p: &std::path::Path, base: &std::path::Path) -> std::path::PathBuf {
    if p.is_absolute() || base.as_os_str().is_empty() {
        p.to_path_buf()
    } else {
        base.join(p)
    }
}

impl GpuRenderer {
    pub fn new() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .map_err(|_| GpuError::NoAdapter)?;

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("rustshade"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            }))?;

        let bgl_render = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rs-std"),
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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bgl_lut = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rs-lut"),
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
                tex_entry(1, wgpu::TextureViewDimension::D2),
                tex_entry(2, wgpu::TextureViewDimension::D3),
                sampler_entry(3),
            ],
        });

        let bgl_composite = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rs-composite"),
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
                tex_entry(1, wgpu::TextureViewDimension::D2),
                tex_entry(2, wgpu::TextureViewDimension::D2),
                sampler_entry(3),
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("rs-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let mut pipelines = HashMap::new();
        for pid in [
            PipelineId::Grade,
            PipelineId::Vignette,
            PipelineId::ChromaticAberration,
            PipelineId::Sharpen,
            PipelineId::Fxaa,
            PipelineId::Grain,
            PipelineId::Scanlines,
            PipelineId::Lut,
            PipelineId::BloomThreshold,
            PipelineId::BloomBlur,
            PipelineId::BloomComposite,
        ] {
            let bgl = match pid.layout() {
                LayoutKind::Standard => &bgl_render,
                LayoutKind::Lut => &bgl_lut,
                LayoutKind::BloomComposite => &bgl_composite,
            };
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[bgl],
                push_constant_ranges: &[],
            });
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(format!("{COMMON_WGSL}\n{}", pid.source()).into()),
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview: Default::default(),
                cache: None,
            });
            pipelines.insert(pid, pipeline);
        }

        Ok(Self {
            device,
            queue,
            pipelines,
            bgl_render,
            bgl_lut,
            bgl_composite,
            sampler,
            tex_a: None,
            tex_b: None,
            small_a: None,
            small_b: None,
            luts: HashMap::new(),
            lut_base: std::env::current_dir().unwrap_or_default(),
        })
    }

    pub fn set_lut_base_dir(&mut self, dir: impl AsRef<std::path::Path>) {
        self.lut_base = dir.as_ref().to_path_buf();
    }

    fn uniform_buffer(&self, block: UniformBlock) -> wgpu::Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &block.bytes(),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
    }

    fn ensure_textures(&mut self, w: u32, h: u32) -> Result<(), GpuError> {
        let needs = match (&self.tex_a, &self.tex_b) {
            (None, _) | (_, None) => true,
            (Some((t, _)), _) => t.width() != w || t.height() != h,
        };
        if !needs {
            return Ok(());
        }
        let small_w = (w / 2).max(1);
        let small_h = (h / 2).max(1);
        self.tex_a = Some(create_rt(&self.device, w, h, "rs-a"));
        self.tex_b = Some(create_rt(&self.device, w, h, "rs-b"));
        self.small_a = Some(create_rt(&self.device, small_w, small_h, "rs-small-a"));
        self.small_b = Some(create_rt(&self.device, small_w, small_h, "rs-small-b"));
        Ok(())
    }

    fn get_lut_str(&mut self, path: &str) -> Result<(u32, wgpu::TextureView), GpuError> {
        if let Some((n, _, view)) = self.luts.get(path) {
            return Ok((*n, view.clone()));
        }
        let lut = Lut3D::load(path)?;
        let n = lut.size();
        let mut bytes = Vec::with_capacity((n * n * n * 4) as usize);
        for entry in lut.raw_data() {
            for c in entry {
                bytes.push((c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            }
            bytes.push(255);
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rs-lut"),
            size: wgpu::Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: n,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(n * 4),
                rows_per_image: Some(n),
            },
            wgpu::Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: n,
            },
        );
        let view = texture.create_view(&Default::default());
        self.luts
            .insert(path.to_string(), (n, texture, view.clone()));
        Ok((n, view))
    }

    /// Applies the effect stack of a preset to an RGBA8 image.
    pub fn apply(
        &mut self,
        effects: &[EffectSpec],
        img: &image::RgbaImage,
        frame_index: u64,
    ) -> Result<image::RgbaImage, GpuError> {
        let w = img.width();
        let h = img.height();
        self.ensure_textures(w, h)?;
        let tex_a = self.tex_a.as_ref().unwrap().0.clone();
        let view_a = self.tex_a.as_ref().unwrap().1.clone();
        let view_b = self.tex_b.as_ref().unwrap().1.clone();

        let img_bytes: Vec<u8> = img.pixels().flat_map(|p| p.0).collect();
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex_a,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &img_bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        let mut cur_view = view_a.clone();
        let mut cur_is_a = true;
        let mut passes = 0usize;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rs-enc"),
            });

        for spec in effects.iter().filter(|s| s.enabled) {
            match spec.id.as_str() {
                "bloom" => {
                    let (_, small_a_view) = self.small_a.as_ref().unwrap();
                    let (_, small_b_view) = self.small_b.as_ref().unwrap();
                    let small_w = self.small_a.as_ref().unwrap().0.width();
                    let small_h = self.small_a.as_ref().unwrap().0.height();

                    let mk = |p0: [f32; 4], p1: [f32; 4], dims: [f32; 2]| UniformBlock {
                        p0,
                        p1,
                        p2: [0.0; 4],
                        dims,
                        frame: frame_index as f32,
                        pad: 0.0,
                    };
                    let strength = get(spec, "strength", 0.5);
                    let radius = get(spec, "radius", 4.0);

                    let ub_threshold = self.uniform_buffer(mk(
                        [
                            get(spec, "threshold", 0.75),
                            get(spec, "knee", 0.3),
                            0.0,
                            0.0,
                        ],
                        [0.0; 4],
                        [w as f32, h as f32],
                    ));
                    let ub_blur_h = self.uniform_buffer(mk(
                        [radius, 0.0, 0.0, 0.0],
                        [0.0, 0.0, 0.0, 0.0],
                        [small_w as f32, small_h as f32],
                    ));
                    let ub_blur_v = self.uniform_buffer(mk(
                        [radius, 0.0, 0.0, 0.0],
                        [1.0, 0.0, 0.0, 0.0],
                        [small_w as f32, small_h as f32],
                    ));
                    let ub_composite = self.uniform_buffer(mk(
                        [strength, 0.0, 0.0, 0.0],
                        [0.0; 4],
                        [w as f32, h as f32],
                    ));

                    let bg_threshold = self.make_bind_group(
                        &self.bgl_render,
                        &ub_threshold,
                        Some(&cur_view),
                        None,
                        None,
                    );
                    let bg_blur_h = self.make_bind_group(
                        &self.bgl_render,
                        &ub_blur_h,
                        Some(small_a_view),
                        None,
                        None,
                    );
                    let bg_blur_v = self.make_bind_group(
                        &self.bgl_render,
                        &ub_blur_v,
                        Some(small_b_view),
                        None,
                        None,
                    );
                    let bg_composite = self.make_bind_group(
                        &self.bgl_composite,
                        &ub_composite,
                        Some(&cur_view),
                        Some(small_a_view),
                        None,
                    );

                    render_pass(
                        &mut encoder,
                        self.pipelines.get(&PipelineId::BloomThreshold).unwrap(),
                        &bg_threshold,
                        small_a_view,
                    );
                    render_pass(
                        &mut encoder,
                        self.pipelines.get(&PipelineId::BloomBlur).unwrap(),
                        &bg_blur_h,
                        small_b_view,
                    );
                    render_pass(
                        &mut encoder,
                        self.pipelines.get(&PipelineId::BloomBlur).unwrap(),
                        &bg_blur_v,
                        small_a_view,
                    );
                    render_pass(
                        &mut encoder,
                        self.pipelines.get(&PipelineId::BloomComposite).unwrap(),
                        &bg_composite,
                        &view_b,
                    );

                    cur_view = view_b.clone();
                    cur_is_a = false;
                    passes += 1;
                }
                "bloom_blur" | "bloom_threshold" | "bloom_composite" => {}
                other => {
                    let Some(pid) = PipelineId::from_effect_id(other) else {
                        continue;
                    };
                    let uniform = uniform_for(spec, w, h, frame_index, [0.0; 4]);
                    let ub = self.uniform_buffer(uniform);

                    let bg = if pid.layout() == LayoutKind::Lut {
                        let path = spec
                            .get_str("path")
                            .ok_or(GpuError::Operation("lut requires a path".into()))?
                            .to_string();
                        let lut_path =
                            resolve_lut_path(std::path::Path::new(&path), self.lut_base.as_path());
                        let (dim, lut_view) =
                            self.get_lut_str(lut_path.to_string_lossy().as_ref())?;
                        let ub2 = self.uniform_buffer(uniform_for(
                            spec,
                            w,
                            h,
                            frame_index,
                            [dim as f32, 0.0, 0.0, 0.0],
                        ));
                        self.make_bind_group(
                            &self.bgl_lut,
                            &ub2,
                            Some(&cur_view),
                            None,
                            Some(&lut_view),
                        )
                    } else {
                        self.make_bind_group(&self.bgl_render, &ub, Some(&cur_view), None, None)
                    };

                    let target = if cur_is_a { &view_b } else { &view_a };
                    render_pass(&mut encoder, self.pipelines.get(&pid).unwrap(), &bg, target);
                    cur_view = target.clone();
                    cur_is_a = !cur_is_a;
                    passes += 1;
                }
            }
        }

        if passes == 0 {
            return Ok(img.clone());
        }

        self.queue.submit([encoder.finish()]);

        // Read back the final texture.
        let final_view = cur_view;
        let _ = final_view;
        let final_tex = if cur_is_a {
            self.tex_a.as_ref().unwrap().0.clone()
        } else {
            self.tex_b.as_ref().unwrap().0.clone()
        };
        let row = (w * 4) as usize;
        let padded = row.div_ceil(256) * 256;
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rs-read"),
            size: (padded * h as usize) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rs-read-enc"),
            });
        encoder.copy_texture_to_buffer(
            final_tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded as u32),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = self.device.poll(wgpu::PollType::wait());
        rx.recv()
            .map_err(|_| GpuError::Operation("map signal lost".into()))?
            .map_err(|e| GpuError::Operation(format!("map failed: {e}")))?;

        let data = slice.get_mapped_range();
        let mut out = image::RgbaImage::new(w, h);
        for y in 0..h as usize {
            let src = &data[y * padded..y * padded + row];
            for x in 0..w as usize {
                let p = image::Rgba([src[x * 4], src[x * 4 + 1], src[x * 4 + 2], src[x * 4 + 3]]);
                out.put_pixel(x as u32, y as u32, p);
            }
        }
        drop(data);
        staging.unmap();
        Ok(out)
    }

    fn make_bind_group(
        &self,
        layout: &wgpu::BindGroupLayout,
        uniform: &wgpu::Buffer,
        tex: Option<&wgpu::TextureView>,
        tex2: Option<&wgpu::TextureView>,
        lut: Option<&wgpu::TextureView>,
    ) -> wgpu::BindGroup {
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        }];
        let mut next_binding = 1;
        if let Some(t) = tex {
            entries.push(wgpu::BindGroupEntry {
                binding: next_binding,
                resource: wgpu::BindingResource::TextureView(t),
            });
            next_binding += 1;
        }
        if let Some(t2) = tex2 {
            entries.push(wgpu::BindGroupEntry {
                binding: next_binding,
                resource: wgpu::BindingResource::TextureView(t2),
            });
            next_binding += 1;
        }
        if let Some(l) = lut {
            entries.push(wgpu::BindGroupEntry {
                binding: next_binding,
                resource: wgpu::BindingResource::TextureView(l),
            });
            next_binding += 1;
        }
        entries.push(wgpu::BindGroupEntry {
            binding: next_binding,
            resource: wgpu::BindingResource::Sampler(&self.sampler),
        });
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &entries,
        })
    }
}

fn tex_entry(binding: u32, dim: wgpu::TextureViewDimension) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: dim,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn create_rt(
    device: &wgpu::Device,
    w: u32,
    h: u32,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    (texture, view)
}

fn render_pass(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    bg: &wgpu::BindGroup,
    target: &wgpu::TextureView,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bg, &[]);
    pass.draw(0..3, 0..1);
}

/// One-shot helper: creates a renderer and applies the stack.
pub fn apply_to_image(
    effects: &[EffectSpec],
    img: &image::RgbaImage,
    frame_index: u64,
) -> Result<image::RgbaImage, GpuError> {
    let mut renderer = GpuRenderer::new()?;
    renderer.apply(effects, img, frame_index)
}
