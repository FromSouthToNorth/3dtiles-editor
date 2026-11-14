use crate::cache::{TileCache, TileContentState};
use glam::{DVec3, DVec4};
use std::collections::HashSet;
use tiles_core::{RefineMode, TilesetDoc};

#[derive(Debug, Clone, Copy)]
pub struct LodParams {
    pub camera_pos: DVec3,
    pub frustum_planes_rel_cam: [DVec4; 6],
    pub viewport_height: f64,
    pub fov_y_rad: f64,
    pub max_sse: f64,
    pub max_requests_per_frame: usize,
    pub frame_number: u64,
}

#[derive(Debug, Default)]
pub struct LodOutput {
    pub render_tiles: Vec<u64>,
    pub load_content: Vec<u64>,
    pub load_external: Vec<u64>,
    pub cancel_tiles: Vec<u64>,
    pub visible_nodes_visited: usize,
}

pub fn evaluate_lod(
    doc: &TilesetDoc,
    cache: &mut TileCache,
    params: &LodParams,
    external_loading: &HashSet<u64>,
) -> LodOutput {
    let mut out = LodOutput::default();
    let mut content_reqs: Vec<(u64, f64)> = Vec::new();
    let mut external_reqs: Vec<(u64, f64)> = Vec::new();
    let mut active_loading: HashSet<u64> = HashSet::new();

    traverse(
        doc,
        doc.root_id,
        cache,
        params,
        external_loading,
        &mut out,
        &mut content_reqs,
        &mut external_reqs,
        &mut active_loading,
    );

    // 按 SSE 从大到小调度加载，保证近处高精度优先
    content_reqs.sort_by(|a, b| b.1.total_cmp(&a.1));
    external_reqs.sort_by(|a, b| b.1.total_cmp(&a.1));

    out.load_content = content_reqs
        .into_iter()
        .take(params.max_requests_per_frame)
        .map(|(id, _)| id)
        .collect();
    out.load_external = external_reqs
        .into_iter()
        .take(params.max_requests_per_frame)
        .map(|(id, _)| id)
        .collect();

    out
}

#[allow(clippy::too_many_arguments)]
fn traverse(
    doc: &TilesetDoc,
    tile_id: u64,
    cache: &mut TileCache,
    params: &LodParams,
    external_loading: &HashSet<u64>,
    out: &mut LodOutput,
    content_reqs: &mut Vec<(u64, f64)>,
    external_reqs: &mut Vec<(u64, f64)>,
    active_loading: &mut HashSet<u64>,
) -> bool {
    let Some(node) = doc.node(tile_id) else {
        return false;
    };
    if !node.visible {
        return false;
    }
    if !node
        .world_bounds
        .intersects_frustum(&params.frustum_planes_rel_cam, params.camera_pos)
    {
        return false;
    }

    out.visible_nodes_visited += 1;
    cache.touch(tile_id, params.frame_number);

    let dist = node.world_bounds.distance_to_point(params.camera_pos).max(1e-3);
    let denom = dist * 2.0 * (params.fov_y_rad * 0.5).tan().max(1e-4);
    let sse = (node.geometric_error * params.viewport_height) / denom;
    let screen_radius_px = (node.world_bounds.radius * params.viewport_height) / denom;

    let has_content = !node.content_paths.is_empty();
    let has_unloaded_ext = node.external_tileset_path.is_some() && !node.external_tileset_loaded;
    let has_children = !node.children.is_empty() || has_unloaded_ext;

    let is_empty_placeholder = match cache.state(tile_id) {
        Some(TileContentState::ReadyEmpty) => true,
        _ => !has_content,
    };

    // 若内容为空占位（如 80B GLB 或纯结构父节点），在屏幕投影半径合理时强制继续细化至有几何层
    let should_refine = has_children
        && (sse > params.max_sse || (is_empty_placeholder && screen_radius_px >= 4.0));

    if !should_refine {
        if has_content {
            match cache.state(tile_id) {
                Some(TileContentState::Ready(_)) => {
                    out.render_tiles.push(tile_id);
                    return true;
                }
                Some(TileContentState::ReadyEmpty) => return true,
                Some(TileContentState::Loading { .. }) => {
                    active_loading.insert(tile_id);
                    return false;
                }
                Some(TileContentState::Failed(_)) => return false,
                Some(TileContentState::Unloaded) | None => {
                    content_reqs.push((tile_id, sse.max(screen_radius_px)));
                    active_loading.insert(tile_id);
                    return false;
                }
            }
        }
        return false;
    }

    // ADD 模式：自身与满足视锥的子节点一并绘制
    if node.effective_refine == RefineMode::Add {
        if has_content {
            match cache.state(tile_id) {
                Some(TileContentState::Ready(_)) => out.render_tiles.push(tile_id),
                Some(TileContentState::ReadyEmpty | TileContentState::Failed(_)) => {}
                Some(TileContentState::Loading { .. }) => {
                    active_loading.insert(tile_id);
                }
                Some(TileContentState::Unloaded) | None => {
                    content_reqs.push((tile_id, sse));
                }
            }
        }
        if has_unloaded_ext && !external_loading.contains(&tile_id) {
            external_reqs.push((tile_id, sse + 10.0));
        }
        for &cid in &node.children {
            traverse(
                doc,
                cid,
                cache,
                params,
                external_loading,
                out,
                content_reqs,
                external_reqs,
                active_loading,
            );
        }
        return true;
    }

    // REPLACE 模式：若外部子树尚未加载，请求加载外部 tileset.json；同时用父节点兜底
    if has_unloaded_ext {
        if !external_loading.contains(&tile_id) {
            external_reqs.push((tile_id, sse + 100.0));
        }
        if has_content {
            match cache.state(tile_id) {
                Some(TileContentState::Ready(_)) => {
                    out.render_tiles.push(tile_id);
                    return true;
                }
                Some(TileContentState::ReadyEmpty) => return false,
                Some(TileContentState::Loading { .. }) => return false,
                _ => {
                    content_reqs.push((tile_id, sse));
                    return false;
                }
            }
        }
        return false;
    }

    // 对所有可见子节点试探：若存在任一可见子节点未 Ready，则保留当前节点兜底防空洞闪烁
    let prev_render_len = out.render_tiles.len();
    let mut all_visible_children_ready = true;
    let mut any_visible_child = false;

    for &cid in &node.children {
        let Some(cnode) = doc.node(cid) else { continue; };
        if !cnode.visible
            || !cnode
                .world_bounds
                .intersects_frustum(&params.frustum_planes_rel_cam, params.camera_pos)
        {
            continue;
        }
        any_visible_child = true;
        let child_ok = traverse(
            doc,
            cid,
            cache,
            params,
            external_loading,
            out,
            content_reqs,
            external_reqs,
            active_loading,
        );
        if !child_ok {
            all_visible_children_ready = false;
        }
    }

    if any_visible_child && !all_visible_children_ready && has_content && !is_empty_placeholder {
        // 子节点尚未就绪，回退绘制父节点（保留已经 Ready 的部分子树或统一回退父级）
        if out.render_tiles.len() == prev_render_len {
            if let Some(TileContentState::Ready(_)) = cache.state(tile_id) {
                out.render_tiles.push(tile_id);
                return true;
            }
        }
    }
    if has_content && !is_empty_placeholder && !any_visible_child {
        if let Some(TileContentState::Ready(_)) = cache.state(tile_id) {
            out.render_tiles.push(tile_id);
            return true;
        }
    }
    all_visible_children_ready || out.render_tiles.len() > prev_render_len
}
