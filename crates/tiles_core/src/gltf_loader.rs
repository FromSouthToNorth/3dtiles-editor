use anyhow::{bail, Context, Result};
use bytemuck::{Pod, Zeroable};
use glam::{DMat4, DVec3, Vec3};
use image::ImageBuffer;
use std::path::Path;

/// 3D Tiles 规范: glTF Y-up -> 3D Tiles Z-up（绕 +X 轴旋转 +90°）
pub const Y_UP_TO_Z_UP: DMat4 = DMat4::from_cols_array(&[
    1.0, 0.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, -1.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 1.0,
]);

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

#[derive(Debug, Clone)]
pub struct DecodedTexture {
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct DecodedPrimitive {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    /// glTF 节点局部变换已包含 Y-up -> Z-up 转换，且相对于 tile 局部坐标系（或含 RTC 偏移）
    pub local_to_tile: DMat4,
    pub base_color_factor: [f32; 4],
    pub base_color_texture: Option<usize>,
    pub local_aabb_min: Vec3,
    pub local_aabb_max: Vec3,
}

#[derive(Debug, Clone)]
pub struct DecodedMesh {
    pub primitives: Vec<DecodedPrimitive>,
    pub textures: Vec<DecodedTexture>,
    pub approx_cpu_bytes: usize,
}

impl DecodedMesh {
    pub fn empty() -> Self {
        Self {
            primitives: Vec::new(),
            textures: Vec::new(),
            approx_cpu_bytes: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.primitives.is_empty()
    }
}

pub fn load_glb_or_gltf(
    bytes: &[u8],
    base_dir: Option<&Path>,
    rtc_center: DVec3,
) -> Result<DecodedMesh> {
    let gltf = gltf::Gltf::from_slice(bytes).context("Failed to parse glTF/GLB container")?;

    // 检查未支持的必需扩展，避免静默渲染错误几何
    for req in gltf.document.extensions_required() {
        if req == "KHR_draco_mesh_compression" {
            bail!("Unsupported glTF extension: KHR_draco_mesh_compression (scheduled for Phase 2)");
        }
    }

    let buffers = gltf::import_buffers(&gltf.document, base_dir, gltf.blob.clone())
        .context("Failed to load glTF buffers")?;
    let raw_images = gltf::import_images(&gltf.document, base_dir, &buffers)
        .context("Failed to decode glTF images")?;

    let mut textures = Vec::with_capacity(raw_images.len());
    let mut approx_bytes = 0usize;
    for img in raw_images {
        let rgba = convert_image_to_rgba8(img);
        approx_bytes += rgba.rgba8.len();
        textures.push(rgba);
    }

    let rtc_mat = DMat4::from_translation(rtc_center);
    let root_tile_from_gltf = rtc_mat * Y_UP_TO_Z_UP;

    let mut primitives = Vec::new();
    let scene_iter: Vec<gltf::Scene> = if let Some(default_scene) = gltf.document.default_scene() {
        vec![default_scene]
    } else {
        gltf.document.scenes().collect()
    };

    if scene_iter.is_empty() {
        for mesh in gltf.document.meshes() {
            append_mesh_primitives(
                &mesh,
                root_tile_from_gltf,
                &buffers,
                &mut primitives,
                &mut approx_bytes,
            )?;
        }
    } else {
        for scene in scene_iter {
            for node in scene.nodes() {
                traverse_gltf_node(
                    &node,
                    root_tile_from_gltf,
                    &buffers,
                    &mut primitives,
                    &mut approx_bytes,
                )?;
            }
        }
    }

    Ok(DecodedMesh {
        primitives,
        textures,
        approx_cpu_bytes: approx_bytes,
    })
}

fn traverse_gltf_node(
    node: &gltf::Node,
    parent_mat: DMat4,
    buffers: &[gltf::buffer::Data],
    out: &mut Vec<DecodedPrimitive>,
    approx_bytes: &mut usize,
) -> Result<()> {
    let cols = node.transform().matrix();
    let local_mat = DMat4::from_cols_array_2d(&[
        [cols[0][0] as f64, cols[0][1] as f64, cols[0][2] as f64, cols[0][3] as f64],
        [cols[1][0] as f64, cols[1][1] as f64, cols[1][2] as f64, cols[1][3] as f64],
        [cols[2][0] as f64, cols[2][1] as f64, cols[2][2] as f64, cols[2][3] as f64],
        [cols[3][0] as f64, cols[3][1] as f64, cols[3][2] as f64, cols[3][3] as f64],
    ]);
    let world_mat = parent_mat * local_mat;

    if let Some(mesh) = node.mesh() {
        append_mesh_primitives(&mesh, world_mat, buffers, out, approx_bytes)?;
    }

    for child in node.children() {
        traverse_gltf_node(&child, world_mat, buffers, out, approx_bytes)?;
    }
    Ok(())
}

fn append_mesh_primitives(
    mesh: &gltf::Mesh,
    local_to_tile: DMat4,
    buffers: &[gltf::buffer::Data],
    out: &mut Vec<DecodedPrimitive>,
    approx_bytes: &mut usize,
) -> Result<()> {
    for prim in mesh.primitives() {
        if prim.mode() != gltf::mesh::Mode::Triangles {
            continue;
        }
        let reader = prim.reader(|buf| buffers.get(buf.index()).map(|d| d.0.as_slice()));
        let Some(pos_iter) = reader.read_positions() else {
            continue;
        };
        let positions: Vec<[f32; 3]> = pos_iter.collect();
        if positions.is_empty() {
            continue;
        }

        let normals: Option<Vec<[f32; 3]>> = reader.read_normals().map(|n| n.collect());
        let uvs: Option<Vec<[f32; 2]>> = reader.read_tex_coords(0).map(|tc| tc.into_f32().collect());

        let mut aabb_min = Vec3::splat(f32::INFINITY);
        let mut aabb_max = Vec3::splat(f32::NEG_INFINITY);
        let mut vertices = Vec::with_capacity(positions.len());

        for (i, pos) in positions.iter().enumerate() {
            let p = Vec3::from_array(*pos);
            aabb_min = aabb_min.min(p);
            aabb_max = aabb_max.max(p);
            let normal = normals
                .as_ref()
                .and_then(|ns| ns.get(i).copied())
                .unwrap_or([0.0, 0.0, 0.0]); // 零法线标记: WGSL 片元着色器自动通过 dpdx/dpdy 计算硬表面法线
            let uv = uvs
                .as_ref()
                .and_then(|ts| ts.get(i).copied())
                .unwrap_or([0.0, 0.0]);
            vertices.push(Vertex {
                position: *pos,
                normal,
                uv,
            });
        }

        let indices: Vec<u32> = match reader.read_indices() {
            Some(iter) => iter.into_u32().collect(),
            None => (0..vertices.len() as u32).collect(),
        };
        if indices.len() < 3 {
            continue;
        }

        let mat = prim.material();
        let pbr = mat.pbr_metallic_roughness();
        let base_color_factor = pbr.base_color_factor();
        let base_color_texture = pbr
            .base_color_texture()
            .map(|info| info.texture().source().index());

        *approx_bytes += vertices.len() * std::mem::size_of::<Vertex>()
            + indices.len() * std::mem::size_of::<u32>();

        out.push(DecodedPrimitive {
            vertices,
            indices,
            local_to_tile,
            base_color_factor,
            base_color_texture,
            local_aabb_min: aabb_min,
            local_aabb_max: aabb_max,
        });
    }
    Ok(())
}

fn convert_image_to_rgba8(img: gltf::image::Data) -> DecodedTexture {
    use gltf::image::Format;
    let w = img.width;
    let h = img.height;
    let rgba8 = match img.format {
        Format::R8G8B8A8 => img.pixels,
        Format::R8G8B8 => {
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for chunk in img.pixels.chunks_exact(3) {
                out.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
            out
        }
        Format::R8 => {
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for &g in &img.pixels {
                out.extend_from_slice(&[g, g, g, 255]);
            }
            out
        }
        Format::R8G8 => {
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for chunk in img.pixels.chunks_exact(2) {
                out.extend_from_slice(&[chunk[0], chunk[0], chunk[0], chunk[1]]);
            }
            out
        }
        _ => {
            // 16-bit/float 等少见格式降级转 RGBA8 占位
            if let Some(buf) = ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(
                1,
                1,
                vec![255, 255, 255, 255],
            ) {
                return DecodedTexture {
                    width: 1,
                    height: 1,
                    rgba8: buf.into_raw(),
                };
            }
            vec![255, 255, 255, 255]
        }
    };
    DecodedTexture {
        width: w,
        height: h,
        rgba8,
    }
}
