use crate::camera::OrbitCamera;
use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use glam::DVec3;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tiles_core::bounds::BOX_EDGE_INDICES;
use tiles_core::{DecodedMesh, PointInstance, TilesetDoc, Vertex};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],
    sun_dir: [f32; 4],
    eye_fwd: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct DrawUniform {
    model_rel_cam: [[f32; 4]; 4],
    base_color_factor: [f32; 4],
    flags: [f32; 4],
}

/// 点云每云 uniform: 与 points.wgsl 的 PointUniform 布局一致
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct PointUniform {
    model_rel_cam: [[f32; 4]; 4],
    viewport: [f32; 4], // (宽px, 高px, 点直径px, 保留)
    tint: [f32; 4],     // (rgb 高亮色, a 混合强度)
}

/// 点云点直径（设备像素）, 屏幕恒定尺寸
const POINT_SIZE_PX: f32 = 2.0;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct LineVertex {
    rel_pos: [f32; 3],
    color: [f32; 4],
}

struct GpuPrimitive {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    local_to_tile: glam::DMat4,
    base_color_factor: [f32; 4],
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

/// 点云瓦片的 GPU 侧资源: 逐点实例缓冲 + 每云 uniform
struct GpuPointCloud {
    instance_buffer: wgpu::Buffer,
    point_count: u32,
    local_to_tile: glam::DMat4,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

struct GpuTileAsset {
    primitives: Vec<GpuPrimitive>,
    point_clouds: Vec<GpuPointCloud>,
    _textures: Vec<wgpu::Texture>,
    gpu_bytes: usize,
    last_used_frame: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RenderStats {
    pub draw_calls: u32,
    pub rendered_tiles: u32,
    pub gpu_tiles_cached: u32,
    pub gpu_memory_mb: f32,
}

pub struct TilesRenderer {
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_view: wgpu::TextureView,
    adapter_label: String,

    frame_uniform_buf: wgpu::Buffer,
    frame_bind_group: wgpu::BindGroup,
    draw_bind_group_layout: wgpu::BindGroupLayout,
    mesh_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    point_pipeline: wgpu::RenderPipeline,
    point_draw_bind_group_layout: wgpu::BindGroupLayout,
    /// billboard 四边形角点缓冲（静态, 6 顶点两三角形）
    point_corner_buf: wgpu::Buffer,

    fallback_tex_view: wgpu::TextureView,
    default_sampler: wgpu::Sampler,

    gpu_tiles: HashMap<u64, GpuTileAsset>,
    total_gpu_bytes: usize,
    gpu_budget_bytes: usize,

    line_vertex_buf: wgpu::Buffer,
    line_vertex_capacity: usize,
}

impl TilesRenderer {
    /// # Safety
    /// 调用方必须确保原生窗口与显示句柄在 Renderer 存活期间保持有效
    pub unsafe fn new(
        raw_display: RawDisplayHandle,
        raw_window: RawWindowHandle,
        width: u32,
        height: u32,
        gpu_budget_bytes: usize,
    ) -> Result<Self> {
        // 优先尝试 Vulkan，其次 DX12，最后才考虑其余后端；使用多适配器枚举确保双显卡环境能选出正确呈现设备
        let candidate_backends: &[wgpu::Backends] = if cfg!(target_os = "windows") {
            &[
                wgpu::Backends::VULKAN,
                wgpu::Backends::DX12,
                wgpu::Backends::GL,
            ]
        } else {
            &[wgpu::Backends::all()]
        };

        let mut init_err: Option<anyhow::Error> = None;
        let mut initialized = None;
        'backend_loop: for &backends in candidate_backends {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends,
                ..Default::default()
            });
            let Ok(surface) = instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: raw_display,
                raw_window_handle: raw_window,
            }) else {
                continue;
            };

            // 1. 先尝试直接通过 request_adapter 请求最高性能适配器
            let mut adapters_to_try = Vec::new();
            if let Some(a) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })) {
                adapters_to_try.push(a);
            }
            // 2. 将系统中枚举到的该后端所有物理/显示适配器加入候选（处理双显卡笔记本核显/独显呈现分离）
            for a in instance.enumerate_adapters(backends) {
                if !adapters_to_try.iter().any(|existing| existing.get_info().name == a.get_info().name) {
                    adapters_to_try.push(a);
                }
            }

            for adapter in adapters_to_try {
                let caps = surface.get_capabilities(&adapter);
                if caps.formats.is_empty() {
                    continue;
                }
                let info = adapter.get_info();
                let dev_res = pollster::block_on(adapter.request_device(
                    &wgpu::DeviceDescriptor {
                        label: Some("3dtiles-editor-device"),
                        required_features: wgpu::Features::empty(),
                        required_limits: wgpu::Limits::default(),
                        memory_hints: Default::default(),
                    },
                    None,
                ));
                match dev_res {
                    Ok((device, queue)) => {
                        let label = format!("{} ({:?})", info.name, info.backend);
                        initialized = Some((instance, surface, adapter, device, queue, label));
                        break 'backend_loop;
                    }
                    Err(e) => {
                        init_err = Some(anyhow::anyhow!("adapter {:?} ({:?}): {e}", info.name, info.backend));
                    }
                }
            }
        }

        let (instance, surface, adapter, device, queue, adapter_label) = initialized
            .ok_or_else(|| init_err.unwrap_or_else(|| anyhow::anyhow!("No compatible GPU adapter found")))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let w = width.max(1);
        let h = height.max(1);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: w,
            height: h,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        let depth_view = create_depth_texture(&device, w, h);

        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let draw_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("draw-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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

        let frame_uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("frame-uniform"),
            contents: bytemuck::bytes_of(&FrameUniform {
                view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
                sun_dir: [0.4, 0.5, 0.77, 0.0],
                eye_fwd: [0.0, 0.0, -1.0, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let frame_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame-bg"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_uniform_buf.as_entire_binding(),
            }],
        });

        let mesh_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mesh-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let mesh_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh-pipe-layout"),
            bind_group_layouts: &[&frame_layout, &draw_bind_group_layout],
            push_constant_ranges: &[],
        });

        let mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mesh-pipeline"),
            layout: Some(&mesh_pipe_layout),
            vertex: wgpu::VertexState {
                module: &mesh_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &mesh_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // 允许双面渲染避免单面翻转地形漏空
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::GreaterEqual, // Reversed-Z
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("line-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("wireframe.wgsl").into()),
        });
        let line_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("line-pipe-layout"),
            bind_group_layouts: &[&frame_layout],
            push_constant_ranges: &[],
        });
        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("line-pipeline"),
            layout: Some(&line_pipe_layout),
            vertex: wgpu::VertexState {
                module: &line_shader,
                entry_point: Some("vs_line"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<LineVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &line_shader,
                entry_point: Some("fs_line"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::GreaterEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let point_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("point-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("points.wgsl").into()),
        });
        let point_draw_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("point-draw-layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    // 片元着色器亦读取 tint (选中高亮混合), 需两阶段可见
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let point_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("point-pipe-layout"),
            bind_group_layouts: &[&frame_layout, &point_draw_bind_group_layout],
            push_constant_ranges: &[],
        });
        // 槽 0 = 逐点实例缓冲 (步进 Instance), 槽 1 = 四边形角点缓冲 (步进 Vertex)
        let point_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("point-pipeline"),
            layout: Some(&point_pipe_layout),
            vertex: wgpu::VertexState {
                module: &point_shader,
                entry_point: Some("vs_point"),
                compilation_options: Default::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<PointInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                            wgpu::VertexAttribute {
                                offset: 12,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Unorm8x4,
                            },
                        ],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: 8,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        }],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &point_shader,
                entry_point: Some("fs_point"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::GreaterEqual, // Reversed-Z
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // billboard 角点: 两三角形覆盖 [-0.5,0.5]^2
        let quad_corners: [[f32; 2]; 6] = [
            [-0.5, -0.5],
            [0.5, -0.5],
            [0.5, 0.5],
            [-0.5, -0.5],
            [0.5, 0.5],
            [-0.5, 0.5],
        ];
        let point_corner_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("point-corner-vbo"),
            contents: bytemuck::cast_slice(&quad_corners),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let fallback_tex = upload_rgba_texture(&device, &queue, 1, 1, &[255, 255, 255, 255]);
        let fallback_tex_view = fallback_tex.create_view(&Default::default());
        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("default-sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let initial_cap = 256usize;
        let line_vertex_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bbox-line-vbo"),
            size: (initial_cap * std::mem::size_of::<LineVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            _instance: instance,
            surface,
            device,
            queue,
            config,
            depth_view,
            adapter_label,
            frame_uniform_buf,
            frame_bind_group,
            draw_bind_group_layout,
            mesh_pipeline,
            line_pipeline,
            point_pipeline,
            point_draw_bind_group_layout,
            point_corner_buf,
            fallback_tex_view,
            default_sampler,
            gpu_tiles: HashMap::new(),
            total_gpu_bytes: 0,
            gpu_budget_bytes,
            line_vertex_buf,
            line_vertex_capacity: initial_cap,
        })
    }

    pub fn adapter_label(&self) -> &str {
        &self.adapter_label
    }

    pub fn viewport_size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.depth_view = create_depth_texture(&self.device, width, height);
    }

    pub fn clear_gpu_cache(&mut self) {
        self.gpu_tiles.clear();
        self.total_gpu_bytes = 0;
    }

    pub fn evict_tile(&mut self, tile_id: u64) {
        if let Some(removed) = self.gpu_tiles.remove(&tile_id) {
            self.total_gpu_bytes = self.total_gpu_bytes.saturating_sub(removed.gpu_bytes);
        }
    }

    pub fn has_gpu_tile(&self, tile_id: u64) -> bool {
        self.gpu_tiles.contains_key(&tile_id)
    }

    pub fn upload_tile(&mut self, tile_id: u64, mesh: &Arc<DecodedMesh>, frame_no: u64) {
        if self.gpu_tiles.contains_key(&tile_id) || mesh.is_empty() {
            return;
        }
        let mut gpu_bytes = 0usize;
        let mut tex_objs = Vec::with_capacity(mesh.textures.len());
        let mut tex_views = Vec::with_capacity(mesh.textures.len());
        for t in &mesh.textures {
            let gpu_t = upload_rgba_texture(&self.device, &self.queue, t.width, t.height, &t.rgba8);
            tex_views.push(gpu_t.create_view(&Default::default()));
            tex_objs.push(gpu_t);
            gpu_bytes += (t.width as usize) * (t.height as usize) * 4;
        }

        let mut primitives = Vec::with_capacity(mesh.primitives.len());
        for prim in &mesh.primitives {
            let vbuf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("tile-vbo"),
                    contents: bytemuck::cast_slice(&prim.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let ibuf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("tile-ibo"),
                    contents: bytemuck::cast_slice(&prim.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
            let ubuf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("tile-ubo"),
                    contents: bytemuck::bytes_of(&DrawUniform {
                        model_rel_cam: glam::Mat4::IDENTITY.to_cols_array_2d(),
                        base_color_factor: prim.base_color_factor,
                        flags: [0.0; 4],
                    }),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

            let tex_view = prim
                .base_color_texture
                .and_then(|idx| tex_views.get(idx))
                .unwrap_or(&self.fallback_tex_view);

            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("tile-prim-bg"),
                layout: &self.draw_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: ubuf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(tex_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.default_sampler),
                    },
                ],
            });

            gpu_bytes += prim.vertices.len() * std::mem::size_of::<Vertex>()
                + prim.indices.len() * std::mem::size_of::<u32>();

            primitives.push(GpuPrimitive {
                vertex_buffer: vbuf,
                index_buffer: ibuf,
                index_count: prim.indices.len() as u32,
                local_to_tile: prim.local_to_tile,
                base_color_factor: prim.base_color_factor,
                uniform_buffer: ubuf,
                bind_group,
            });
        }

        // pnts 点云: 逐点实例缓冲 (位置 + RGBA8), 每云一个 uniform
        let mut point_clouds = Vec::with_capacity(mesh.points.len());
        for cloud in &mesh.points {
            let instance_buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("point-cloud-instances"),
                    contents: bytemuck::cast_slice(&cloud.instances),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let ubuf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("point-cloud-ubo"),
                    contents: bytemuck::bytes_of(&PointUniform {
                        model_rel_cam: glam::Mat4::IDENTITY.to_cols_array_2d(),
                        viewport: [
                            self.config.width as f32,
                            self.config.height as f32,
                            POINT_SIZE_PX,
                            0.0,
                        ],
                        tint: [1.0, 1.0, 1.0, 0.0],
                    }),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("point-cloud-bg"),
                layout: &self.point_draw_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ubuf.as_entire_binding(),
                }],
            });
            gpu_bytes += cloud.instances.len() * std::mem::size_of::<PointInstance>();
            point_clouds.push(GpuPointCloud {
                instance_buffer,
                point_count: cloud.instances.len() as u32,
                local_to_tile: cloud.local_to_tile,
                uniform_buffer: ubuf,
                bind_group,
            });
        }

        self.total_gpu_bytes += gpu_bytes;
        self.gpu_tiles.insert(
            tile_id,
            GpuTileAsset {
                primitives,
                point_clouds,
                _textures: tex_objs,
                gpu_bytes,
                last_used_frame: frame_no,
            },
        );
    }

    pub fn evict_gpu_over_budget(&mut self, pinned: &HashSet<u64>) {
        if self.total_gpu_bytes <= self.gpu_budget_bytes {
            return;
        }
        let mut lru: Vec<(u64, u64)> = self
            .gpu_tiles
            .iter()
            .filter(|(id, _)| !pinned.contains(id))
            .map(|(&id, a)| (id, a.last_used_frame))
            .collect();
        lru.sort_by_key(|&(_, f)| f);

        for (id, _) in lru {
            if self.total_gpu_bytes <= self.gpu_budget_bytes {
                break;
            }
            self.evict_tile(id);
        }
    }

    pub fn render(
        &mut self,
        camera: &OrbitCamera,
        doc: Option<&TilesetDoc>,
        render_tile_ids: &[u64],
        selected_tile_id: Option<u64>,
        show_all_bboxes: bool,
        frame_no: u64,
    ) -> Result<RenderStats> {
        if self.config.width == 0 || self.config.height == 0 {
            return Ok(RenderStats::default());
        }

        let aspect = (self.config.width as f64) / (self.config.height.max(1) as f64);
        let vp_f32 = camera.view_proj_rel_cam_f32(aspect);
        let eye_world = camera.eye_world();
        let (_, _, cam_fwd) = camera.camera_axes();
        let sun_dir = (camera.local_up * 0.75 + cam_fwd * -0.35 + DVec3::new(0.2, 0.15, 0.25))
            .normalize();

        self.queue.write_buffer(
            &self.frame_uniform_buf,
            0,
            bytemuck::bytes_of(&FrameUniform {
                view_proj: vp_f32.to_cols_array_2d(),
                sun_dir: [sun_dir.x as f32, sun_dir.y as f32, sun_dir.z as f32, 0.0],
                eye_fwd: [cam_fwd.x as f32, cam_fwd.y as f32, cam_fwd.z as f32, 0.0],
            }),
        );

        let mut draw_calls = 0u32;
        let mut rendered_tiles = 0u32;

        if let Some(doc) = doc {
            for &tid in render_tile_ids {
                let Some(node) = doc.node(tid) else { continue; };
                let Some(gpu_asset) = self.gpu_tiles.get_mut(&tid) else { continue; };
                gpu_asset.last_used_frame = frame_no;
                rendered_tiles += 1;
                let is_sel = selected_tile_id == Some(tid);

                for prim in &gpu_asset.primitives {
                    let mut world_mat = node.world_transform * prim.local_to_tile;
                    world_mat.w_axis -= eye_world.extend(0.0);
                    let model_f32 = world_mat.as_mat4();
                    self.queue.write_buffer(
                        &prim.uniform_buffer,
                        0,
                        bytemuck::bytes_of(&DrawUniform {
                            model_rel_cam: model_f32.to_cols_array_2d(),
                            base_color_factor: prim.base_color_factor,
                            flags: [if is_sel { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
                        }),
                    );
                    draw_calls += 1;
                }

                // 点云 uniform: RTE 同样在 CPU 侧 f64 扣除视点
                for cloud in &gpu_asset.point_clouds {
                    let mut world_mat = node.world_transform * cloud.local_to_tile;
                    world_mat.w_axis -= eye_world.extend(0.0);
                    let tint: [f32; 4] = if is_sel {
                        [0.98, 0.75, 0.14, 0.55] // 选中高亮: 金橙 55% 混合
                    } else {
                        [1.0, 1.0, 1.0, 0.0]
                    };
                    self.queue.write_buffer(
                        &cloud.uniform_buffer,
                        0,
                        bytemuck::bytes_of(&PointUniform {
                            model_rel_cam: world_mat.as_mat4().to_cols_array_2d(),
                            viewport: [
                                self.config.width as f32,
                                self.config.height as f32,
                                POINT_SIZE_PX,
                                0.0,
                            ],
                            tint,
                        }),
                    );
                    draw_calls += 1;
                }
            }
        }

        // 构造包围盒线框顶点（选中节点高亮金橙，可选显示渲染瓦片青色框）
        let mut line_verts = Vec::new();
        if let Some(doc) = doc {
            if show_all_bboxes {
                for &tid in render_tile_ids {
                    if selected_tile_id == Some(tid) {
                        continue;
                    }
                    if let Some(node) = doc.node(tid) {
                        push_box_lines(
                            &mut line_verts,
                            &node.world_bounds.corners(),
                            eye_world,
                            [0.15, 0.72, 0.90, 0.45],
                        );
                    }
                }
            }
            if let Some(sel_id) = selected_tile_id {
                if let Some(node) = doc.node(sel_id) {
                    push_box_lines(
                        &mut line_verts,
                        &node.world_bounds.corners(),
                        eye_world,
                        [0.98, 0.75, 0.14, 1.0],
                    );
                }
            }
        }

        if line_verts.len() > self.line_vertex_capacity {
            let new_cap = line_verts.len().next_power_of_two();
            self.line_vertex_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("bbox-line-vbo"),
                size: (new_cap * std::mem::size_of::<LineVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.line_vertex_capacity = new_cap;
        }
        if !line_verts.is_empty() {
            self.queue
                .write_buffer(&self.line_vertex_buf, 0, bytemuck::cast_slice(&line_verts));
        }

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                match self.surface.get_current_texture() {
                    Ok(f) => f,
                    Err(_) => return Ok(RenderStats::default()),
                }
            }
            Err(e) => return Err(e.into()),
        };

        let color_view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tiles-main-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.055,
                            g: 0.075,
                            b: 0.105,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0), // Reversed-Z 远平面 0.0
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if doc.is_some() {
                rpass.set_pipeline(&self.mesh_pipeline);
                rpass.set_bind_group(0, &self.frame_bind_group, &[]);
                for &tid in render_tile_ids {
                    let Some(gpu_asset) = self.gpu_tiles.get(&tid) else { continue; };
                    for prim in &gpu_asset.primitives {
                        rpass.set_bind_group(1, &prim.bind_group, &[]);
                        rpass.set_vertex_buffer(0, prim.vertex_buffer.slice(..));
                        rpass.set_index_buffer(prim.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        rpass.draw_indexed(0..prim.index_count, 0, 0..1);
                    }
                }
            }

            // 点云绘制块: 实例化 billboard, 槽 0 = 逐点实例, 槽 1 = 静态角点
            if doc.is_some() {
                rpass.set_pipeline(&self.point_pipeline);
                rpass.set_bind_group(0, &self.frame_bind_group, &[]);
                rpass.set_vertex_buffer(1, self.point_corner_buf.slice(..));
                for &tid in render_tile_ids {
                    let Some(gpu_asset) = self.gpu_tiles.get(&tid) else { continue; };
                    for cloud in &gpu_asset.point_clouds {
                        rpass.set_bind_group(1, &cloud.bind_group, &[]);
                        rpass.set_vertex_buffer(0, cloud.instance_buffer.slice(..));
                        rpass.draw(0..6, 0..cloud.point_count);
                    }
                }
            }

            if !line_verts.is_empty() {
                rpass.set_pipeline(&self.line_pipeline);
                rpass.set_bind_group(0, &self.frame_bind_group, &[]);
                rpass.set_vertex_buffer(0, self.line_vertex_buf.slice(..));
                rpass.draw(0..line_verts.len() as u32, 0..1);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();

        Ok(RenderStats {
            draw_calls,
            rendered_tiles,
            gpu_tiles_cached: self.gpu_tiles.len() as u32,
            gpu_memory_mb: self.total_gpu_bytes as f32 / (1024.0 * 1024.0),
        })
    }
}

fn push_box_lines(out: &mut Vec<LineVertex>, corners: &[DVec3; 8], eye: DVec3, color: [f32; 4]) {
    for &(a, b) in &BOX_EDGE_INDICES {
        let pa = (corners[a] - eye).as_vec3().to_array();
        let pb = (corners[b] - eye).as_vec3().to_array();
        out.push(LineVertex { rel_pos: pa, color });
        out.push(LineVertex { rel_pos: pb, color });
    }
}

fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth-reversed-z"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&Default::default())
}

fn upload_rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> wgpu::Texture {
    let w = width.max(1);
    let h = height.max(1);
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tile-base-color"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * w),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    tex
}
