use crate::bounds::{BoundingVolume, WorldBounds};
use crate::gltf_loader::DecodedMesh;
use crate::schema::{
    compose_trs, decompose_trs, resolve_tile_uri, RefineMode, TileJson, TilesetJson, Trs,
};
use anyhow::{bail, Context, Result};
use glam::{DMat4, DVec3};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TilesetSourceFile {
    pub path: PathBuf,
    pub base_dir: PathBuf,
    pub ast: TilesetJson,
    pub dirty: bool,
}

#[derive(Debug, Clone)]
pub struct TileNode {
    pub id: u64,
    pub parent: Option<u64>,
    pub children: Vec<u64>,
    pub label: String,
    pub source_file_idx: usize,
    pub ast_child_path: Vec<usize>,
    pub local_transform: DMat4,
    pub world_transform: DMat4,
    pub effective_refine: RefineMode,
    pub geometric_error: f64,
    pub local_bv: BoundingVolume,
    pub world_bounds: WorldBounds,
    pub content_paths: Vec<PathBuf>,
    pub external_tileset_path: Option<PathBuf>,
    pub external_tileset_loaded: bool,
    pub visible: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TileSummaryDto {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub label: String,
    pub child_count: usize,
    pub has_content: bool,
    pub has_external_tileset: bool,
    pub geometric_error: f64,
    pub refine: String,
    pub bv_kind: String,
    pub bv_center: [f64; 3],
    pub bv_radius: f64,
    pub visible: bool,
    pub trs: Trs,
}

#[derive(Debug, Clone, Copy)]
pub struct FocusPose {
    pub target: DVec3,
    pub radius: f64,
    pub local_up: DVec3,
}

#[derive(Debug, Clone)]
pub struct PickHit {
    pub tile_id: u64,
    pub distance: f64,
    pub world_point: DVec3,
}

pub struct TilesetDoc {
    pub root_id: u64,
    pub source_files: Vec<TilesetSourceFile>,
    nodes: Vec<TileNode>,
    id_to_index: HashMap<u64, usize>,
    next_id: u64,
}

impl TilesetDoc {
    pub fn open_file(path: &Path) -> Result<Self> {
        let canonical = path.to_path_buf();
        let base_dir = canonical
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let text = std::fs::read_to_string(&canonical)
            .with_context(|| format!("Failed to read tileset JSON: {}", canonical.display()))?;
        let ast: TilesetJson = serde_json::from_str(&text)
            .with_context(|| format!("Invalid tileset JSON: {}", canonical.display()))?;

        let mut doc = Self {
            root_id: 0,
            source_files: vec![TilesetSourceFile {
                path: canonical,
                base_dir: base_dir.clone(),
                ast: ast.clone(),
                dirty: false,
            }],
            nodes: Vec::new(),
            id_to_index: HashMap::new(),
            next_id: 1,
        };

        let root_refine = ast.root.refine.unwrap_or(RefineMode::Replace);
        let root_id = doc.insert_subtree(
            None,
            0,
            Vec::new(),
            &base_dir,
            &ast.root,
            DMat4::IDENTITY,
            root_refine,
            "root".to_string(),
        );
        doc.root_id = root_id;
        Ok(doc)
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_subtree(
        &mut self,
        parent: Option<u64>,
        source_file_idx: usize,
        ast_child_path: Vec<usize>,
        base_dir: &Path,
        tile_json: &TileJson,
        parent_world: DMat4,
        inherited_refine: RefineMode,
        fallback_label: String,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let local_transform = tile_json.local_matrix();
        let world_transform = parent_world * local_transform;
        let effective_refine = tile_json.refine.unwrap_or(inherited_refine);
        let local_bv = BoundingVolume::from_json(&tile_json.bounding_volume);
        let world_bounds = local_bv.to_world_bounds(world_transform);

        let mut content_paths = Vec::new();
        let mut external_tileset_path = None;
        let mut label = fallback_label;

        for item in tile_json.content_items() {
            let resolved = resolve_tile_uri(base_dir, &item.uri);
            if item.uri.to_ascii_lowercase().ends_with(".json") {
                external_tileset_path = Some(resolved);
                label = item.uri.clone();
            } else {
                if !item.uri.is_empty() {
                    label = item.uri.clone();
                }
                content_paths.push(resolved);
            }
        }

        let idx = self.nodes.len();
        self.nodes.push(TileNode {
            id,
            parent,
            children: Vec::new(),
            label,
            source_file_idx,
            ast_child_path: ast_child_path.clone(),
            local_transform,
            world_transform,
            effective_refine,
            geometric_error: tile_json.geometric_error,
            local_bv,
            world_bounds,
            content_paths,
            external_tileset_path,
            external_tileset_loaded: false,
            visible: tile_json.is_editor_visible(),
        });
        self.id_to_index.insert(id, idx);

        let mut child_ids = Vec::with_capacity(tile_json.children.len());
        for (c_idx, child_json) in tile_json.children.iter().enumerate() {
            let mut child_path = ast_child_path.clone();
            child_path.push(c_idx);
            let cid = self.insert_subtree(
                Some(id),
                source_file_idx,
                child_path,
                base_dir,
                child_json,
                world_transform,
                effective_refine,
                format!("tile_{c_idx}"),
            );
            child_ids.push(cid);
        }
        self.nodes[idx].children = child_ids;
        id
    }

    /// 将已异步读取的外部 tileset.json 挂载为 parent_id 的子节点
    pub fn splice_external_tileset(
        &mut self,
        parent_id: u64,
        ext_path: PathBuf,
        ast: TilesetJson,
    ) -> Result<u64> {
        let Some(&parent_idx) = self.id_to_index.get(&parent_id) else {
            bail!("Parent tile {parent_id} not found");
        };
        if self.nodes[parent_idx].external_tileset_loaded {
            return Ok(self.nodes[parent_idx].children[0]);
        }

        let parent_world = self.nodes[parent_idx].world_transform;
        let parent_refine = self.nodes[parent_idx].effective_refine;
        let base_dir = ext_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        let src_idx = self.source_files.len();
        let root_json = ast.root.clone();
        self.source_files.push(TilesetSourceFile {
            path: ext_path.clone(),
            base_dir: base_dir.clone(),
            ast,
            dirty: false,
        });

        let ext_name = ext_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("external_root")
            .to_string();
        let child_root_id = self.insert_subtree(
            Some(parent_id),
            src_idx,
            Vec::new(),
            &base_dir,
            &root_json,
            parent_world,
            parent_refine,
            ext_name,
        );

        self.nodes[parent_idx].children.push(child_root_id);
        self.nodes[parent_idx].external_tileset_loaded = true;
        Ok(child_root_id)
    }

    pub fn node(&self, id: u64) -> Option<&TileNode> {
        self.id_to_index.get(&id).map(|&idx| &self.nodes[idx])
    }

    pub fn all_nodes(&self) -> &[TileNode] {
        &self.nodes
    }

    pub fn set_tile_trs(&mut self, id: u64, trs: Trs) -> Result<()> {
        let Some(&idx) = self.id_to_index.get(&id) else {
            bail!("Tile {id} not found");
        };
        let mat = compose_trs(trs);
        self.nodes[idx].local_transform = mat;

        let src_idx = self.nodes[idx].source_file_idx;
        let ast_path = self.nodes[idx].ast_child_path.clone();
        if let Some(tile_ast) = Self::ast_node_mut(&mut self.source_files[src_idx].ast.root, &ast_path) {
            tile_ast.set_transform_matrix(mat);
            self.source_files[src_idx].dirty = true;
        }

        let parent_world = self.nodes[idx]
            .parent
            .and_then(|pid| self.node(pid))
            .map(|p| p.world_transform)
            .unwrap_or(DMat4::IDENTITY);
        self.recompute_world_recursive(id, parent_world);
        Ok(())
    }

    pub fn set_tile_visible(&mut self, id: u64, visible: bool) -> Result<()> {
        let Some(&idx) = self.id_to_index.get(&id) else {
            bail!("Tile {id} not found");
        };
        self.nodes[idx].visible = visible;

        let src_idx = self.nodes[idx].source_file_idx;
        let ast_path = self.nodes[idx].ast_child_path.clone();
        if let Some(tile_ast) = Self::ast_node_mut(&mut self.source_files[src_idx].ast.root, &ast_path) {
            tile_ast.set_editor_visible(visible);
            self.source_files[src_idx].dirty = true;
        }
        Ok(())
    }

    fn recompute_world_recursive(&mut self, id: u64, parent_world: DMat4) {
        let Some(&idx) = self.id_to_index.get(&id) else {
            return;
        };
        let world = parent_world * self.nodes[idx].local_transform;
        self.nodes[idx].world_transform = world;
        self.nodes[idx].world_bounds = self.nodes[idx].local_bv.to_world_bounds(world);
        let children = self.nodes[idx].children.clone();
        for cid in children {
            self.recompute_world_recursive(cid, world);
        }
    }

    fn ast_node_mut<'a>(root: &'a mut TileJson, path: &[usize]) -> Option<&'a mut TileJson> {
        let mut cur = root;
        for &c_idx in path {
            cur = cur.children.get_mut(c_idx)?;
        }
        Some(cur)
    }

    pub fn summary(&self, id: u64) -> Option<TileSummaryDto> {
        let n = self.node(id)?;
        Some(TileSummaryDto {
            id: n.id,
            parent_id: n.parent,
            label: n.label.clone(),
            child_count: n.children.len(),
            has_content: !n.content_paths.is_empty(),
            has_external_tileset: n.external_tileset_path.is_some(),
            geometric_error: n.geometric_error,
            refine: match n.effective_refine {
                RefineMode::Add => "ADD".to_string(),
                RefineMode::Replace => "REPLACE".to_string(),
            },
            bv_kind: n.local_bv.kind_name().to_string(),
            bv_center: n.world_bounds.center.to_array(),
            bv_radius: n.world_bounds.radius,
            visible: n.visible,
            trs: decompose_trs(n.local_transform),
        })
    }

    pub fn children_summaries(&self, parent_id: u64) -> Vec<TileSummaryDto> {
        let Some(p) = self.node(parent_id) else {
            return Vec::new();
        };
        p.children
            .iter()
            .filter_map(|&cid| self.summary(cid))
            .collect()
    }

    /// 根据根节点世界包围体和局部 Z 轴（或 ECEF 法线）计算最佳相机初始定位姿态
    pub fn compute_focus_pose(&self, target_id: Option<u64>) -> FocusPose {
        let id = target_id.unwrap_or(self.root_id);
        let Some(node) = self.node(id) else {
            return FocusPose {
                target: DVec3::ZERO,
                radius: 100.0,
                local_up: DVec3::Z,
            };
        };
        let target = node.world_bounds.center;
        let radius = node.world_bounds.radius.max(1.0);
        // 优先使用节点世界变换的局部 +Z 作为上向量（适配 gygcdm 的 ENU->ECEF 倾斜地平线）；
        // 若世界坐标位于地球表面（> 1e6 m），局部 +Z 与地心法线一致。
        let z_col = node.world_transform.z_axis.truncate();
        let local_up = if z_col.length_squared() > 1e-8 {
            z_col.normalize()
        } else if target.length() > 1e6 {
            target.normalize()
        } else {
            DVec3::Z
        };
        FocusPose {
            target,
            radius,
            local_up,
        }
    }

    /// 两级射线拾取: 1) 遍历当前可见渲染/树节点包围体粗筛; 2) 对已解码的 CPU Mesh 做三角形精确求交
    pub fn pick_ray<F>(
        &self,
        ray_origin: DVec3,
        ray_dir: DVec3,
        candidate_tile_ids: &[u64],
        mesh_lookup: &F,
    ) -> Option<PickHit>
    where
        F: Fn(u64) -> Option<std::sync::Arc<DecodedMesh>>,
    {
        let dir = ray_dir.normalize();
        let mut candidates: Vec<(u64, f64)> = Vec::new();
        for &tid in candidate_tile_ids {
            let Some(node) = self.node(tid) else { continue; };
            if !node.visible {
                continue;
            }
            if let Some(t_box) = node.world_bounds.intersect_ray(ray_origin, dir) {
                candidates.push((tid, t_box));
            }
        }
        candidates.sort_by(|a, b| a.1.total_cmp(&b.1));

        let mut best: Option<PickHit> = None;
        for (tid, box_t) in candidates {
            if let Some(ref b) = best {
                if box_t > b.distance {
                    break;
                }
            }
            let Some(node) = self.node(tid) else { continue; };
            if let Some(mesh) = mesh_lookup(tid) {
                for prim in &mesh.primitives {
                    let model_world = node.world_transform * prim.local_to_tile;
                    let inv_model = model_world.inverse();
                    let local_o = inv_model.transform_point3(ray_origin);
                    let local_d = inv_model.transform_vector3(dir);

                    if !intersect_aabb(
                        local_o,
                        local_d,
                        prim.local_aabb_min.as_dvec3(),
                        prim.local_aabb_max.as_dvec3(),
                    ) {
                        continue;
                    }

                    for tri in prim.indices.chunks_exact(3) {
                        let p0 = glam::Vec3::from_array(prim.vertices[tri[0] as usize].position).as_dvec3();
                        let p1 = glam::Vec3::from_array(prim.vertices[tri[1] as usize].position).as_dvec3();
                        let p2 = glam::Vec3::from_array(prim.vertices[tri[2] as usize].position).as_dvec3();
                        if let Some(t_local) = moller_trumbore(local_o, local_d, p0, p1, p2) {
                            let hit_world = model_world.transform_point3(local_o + local_d * t_local);
                            let dist = hit_world.distance(ray_origin);
                            if best.as_ref().map(|b| dist < b.distance).unwrap_or(true) {
                                best = Some(PickHit {
                                    tile_id: tid,
                                    distance: dist,
                                    world_point: hit_world,
                                });
                            }
                        }
                    }
                }
            } else {
                // 若网格不在 CPU 缓存中，降级命中其包围体
                let hit_world = ray_origin + dir * box_t;
                if best.as_ref().map(|b| box_t < b.distance).unwrap_or(true) {
                    best = Some(PickHit {
                        tile_id: tid,
                        distance: box_t,
                        world_point: hit_world,
                    });
                }
            }
        }
        best
    }

    /// 保存所有已修改的 tileset.json（或另存根 tileset 到指定路径）
    pub fn save(&mut self, override_root_path: Option<&Path>) -> Result<Vec<PathBuf>> {
        let mut saved = Vec::new();
        if let Some(out_path) = override_root_path {
            if let Some(root_src) = self.source_files.first_mut() {
                let json = serde_json::to_string_pretty(&root_src.ast)?;
                std::fs::write(out_path, json)?;
                root_src.path = out_path.to_path_buf();
                root_src.dirty = false;
                saved.push(out_path.to_path_buf());
            }
            return Ok(saved);
        }

        for src in &mut self.source_files {
            if src.dirty {
                let json = serde_json::to_string_pretty(&src.ast)?;
                std::fs::write(&src.path, json)
                    .with_context(|| format!("Failed to save {}", src.path.display()))?;
                src.dirty = false;
                saved.push(src.path.clone());
            }
        }
        Ok(saved)
    }
}

fn intersect_aabb(o: DVec3, d: DVec3, min: DVec3, max: DVec3) -> bool {
    let mut tmin = 0.0_f64;
    let mut tmax = f64::INFINITY;
    for i in 0..3 {
        if d[i].abs() < 1e-12 {
            if o[i] < min[i] || o[i] > max[i] {
                return false;
            }
        } else {
            let inv = 1.0 / d[i];
            let mut t1 = (min[i] - o[i]) * inv;
            let mut t2 = (max[i] - o[i]) * inv;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            tmin = tmin.max(t1);
            tmax = tmax.min(t2);
            if tmin > tmax {
                return false;
            }
        }
    }
    true
}

fn moller_trumbore(orig: DVec3, dir: DVec3, v0: DVec3, v1: DVec3, v2: DVec3) -> Option<f64> {
    let e1 = v1 - v0;
    let e2 = v2 - v0;
    let pvec = dir.cross(e2);
    let det = e1.dot(pvec);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv_det = 1.0 / det;
    let tvec = orig - v0;
    let u = tvec.dot(pvec) * inv_det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qvec = tvec.cross(e1);
    let v = dir.dot(qvec) * inv_det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(qvec) * inv_det;
    if t >= 0.0 {
        Some(t)
    } else {
        None
    }
}
