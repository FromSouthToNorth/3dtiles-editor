use crate::commands::{
    AppCommand, CommandHistory, SetTileTrsCommand, SetTileVisibleCommand,
};
use crate::events::EditorEvent;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tiles_core::{TileSummaryDto, TilesetDoc};
use tiles_io::{evaluate_lod, AsyncTileLoader, LoadCompleted, LodParams, TileCache, TileContentState};
use tiles_render::{OrbitCamera, TilesRenderer};

pub struct EngineSession {
    cmd_tx: Sender<AppCommand>,
    evt_rx: Receiver<EditorEvent>,
    doc_shared: Arc<RwLock<Option<TilesetDoc>>>,
    worker_thread: Option<JoinHandle<()>>,
}

impl EngineSession {
    /// 启动渲染与调度主循环线程（传入已由主线程创建的 TilesRenderer，或 None 用于无界面测试）
    pub fn start(renderer: Option<TilesRenderer>) -> anyhow::Result<Self> {
        let (cmd_tx, cmd_rx) = unbounded::<AppCommand>();
        let (evt_tx, evt_rx) = unbounded::<EditorEvent>();
        let doc_shared = Arc::new(RwLock::new(None));
        let doc_for_thread = doc_shared.clone();
        let loader = AsyncTileLoader::new(8)?;

        let handle = thread::Builder::new()
            .name("tiles-engine-loop".to_string())
            .spawn(move || {
                run_engine_loop(renderer, loader, doc_for_thread, cmd_rx, evt_tx);
            })?;

        Ok(Self {
            cmd_tx,
            evt_rx,
            doc_shared,
            worker_thread: Some(handle),
        })
    }

    pub fn send(&self, cmd: AppCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn poll_event(&self) -> Option<EditorEvent> {
        self.evt_rx.try_recv().ok()
    }

    pub fn tile_summary(&self, tile_id: u64) -> Option<TileSummaryDto> {
        let guard = self.doc_shared.read().ok()?;
        let doc = guard.as_ref()?;
        doc.summary(tile_id)
    }

    pub fn tile_children(&self, parent_id: u64) -> Vec<TileSummaryDto> {
        let Ok(guard) = self.doc_shared.read() else {
            return Vec::new();
        };
        let Some(doc) = guard.as_ref() else {
            return Vec::new();
        };
        doc.children_summaries(parent_id)
    }

    pub fn root_summary(&self) -> Option<TileSummaryDto> {
        let guard = self.doc_shared.read().ok()?;
        let doc = guard.as_ref()?;
        doc.summary(doc.root_id)
    }
}

impl Drop for EngineSession {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(AppCommand::Shutdown);
        if let Some(h) = self.worker_thread.take() {
            let _ = h.join();
        }
    }
}

fn push_log(evt_tx: &Sender<EditorEvent>, level: &str, msg: impl Into<String>) {
    let _ = evt_tx.send(EditorEvent::Log {
        level: level.to_string(),
        message: msg.into(),
    });
}

fn run_engine_loop(
    mut renderer: Option<TilesRenderer>,
    loader: AsyncTileLoader,
    doc_shared: Arc<RwLock<Option<TilesetDoc>>>,
    cmd_rx: Receiver<AppCommand>,
    evt_tx: Sender<EditorEvent>,
) {
    let mut cache = TileCache::new(1024 * 1024 * 1024); // 1 GB CPU 缓存预算
    let mut camera = OrbitCamera::default();
    let mut history = CommandHistory::default();
    let mut selected_tile: Option<u64> = None;
    let mut max_sse: f64 = 8.0;
    let mut show_all_bboxes = false;
    let mut external_loading: HashSet<u64> = HashSet::new();
    let mut last_render_tiles: Vec<u64> = Vec::new();
    let mut frame_no: u64 = 0;

    let mut last_stats_time = Instant::now();
    let mut frames_since_stats: u32 = 0;

    let gpu_desc = renderer
        .as_ref()
        .map(|r| r.adapter_label().to_string())
        .unwrap_or_else(|| "headless".to_string());
    push_log(&evt_tx, "info", format!("3D Tiles 引擎线程就绪 ({gpu_desc})"));

    'main: loop {
        let frame_start = Instant::now();

        // 1. 批量消费 UI 命令
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                AppCommand::Shutdown => break 'main,
                AppCommand::Resize { width, height } => {
                    if let Some(r) = renderer.as_mut() {
                        r.resize(width, height);
                    }
                }
                AppCommand::Orbit { dx, dy } => camera.orbit(dx, dy),
                AppCommand::Pan { dx, dy } => {
                    let h = renderer
                        .as_ref()
                        .map(|r| r.viewport_size().1 as f64)
                        .unwrap_or(720.0);
                    camera.pan(dx, dy, h);
                }
                AppCommand::Zoom { delta } => camera.zoom(delta),
                AppCommand::SetMaxSse(sse) => max_sse = sse.clamp(1.0, 64.0),
                AppCommand::SetShowAllBBoxes(v) => show_all_bboxes = v,
                AppCommand::OpenTileset(path) => {
                    let _ = evt_tx.send(EditorEvent::Progress {
                        active: true,
                        progress: 0.1,
                        text: format!("正在打开 {}", path.display()),
                    });
                    loader.bump_generation();
                    cache.clear();
                    external_loading.clear();
                    history.clear();
                    last_render_tiles.clear();
                    selected_tile = None;
                    if let Some(r) = renderer.as_mut() {
                        r.clear_gpu_cache();
                    }

                    match TilesetDoc::open_file(&path) {
                        Ok(doc) => {
                            let root_id = doc.root_id;
                            let node_count = doc.all_nodes().len();
                            let pose = doc.compute_focus_pose(None);
                            camera.focus_on(pose);
                            selected_tile = Some(root_id);
                            let root_summary = doc.summary(root_id);

                            if let Ok(mut w) = doc_shared.write() {
                                *w = Some(doc);
                            }
                            let _ = evt_tx.send(EditorEvent::TilesetOpened {
                                root_id,
                                path: path.display().to_string(),
                                node_count,
                            });
                            let _ = evt_tx.send(EditorEvent::SelectionChanged {
                                summary: root_summary,
                            });
                            push_log(
                                &evt_tx,
                                "info",
                                format!("已加载 tileset: {} (初始节点 {})", path.display(), node_count),
                            );
                        }
                        Err(e) => {
                            let _ = evt_tx.send(EditorEvent::Error {
                                message: format!("打开 tileset 失败: {e:#}"),
                            });
                            push_log(&evt_tx, "error", format!("打开失败: {e:#}"));
                        }
                    }
                    let _ = evt_tx.send(EditorEvent::Progress {
                        active: false,
                        progress: 1.0,
                        text: String::new(),
                    });
                }
                AppCommand::SaveTileset(override_path) => {
                    if let Ok(mut w) = doc_shared.write() {
                        if let Some(doc) = w.as_mut() {
                            match doc.save(override_path.as_deref()) {
                                Ok(files) => {
                                    let list: Vec<String> =
                                        files.iter().map(|p| p.display().to_string()).collect();
                                    let msg = if list.is_empty() {
                                        "无未保存更改".to_string()
                                    } else {
                                        format!("已保存 {} 个文件", list.len())
                                    };
                                    push_log(&evt_tx, "info", msg);
                                    let _ = evt_tx.send(EditorEvent::TilesetSaved { files: list });
                                }
                                Err(e) => {
                                    let _ = evt_tx.send(EditorEvent::Error {
                                        message: format!("保存失败: {e:#}"),
                                    });
                                }
                            }
                        }
                    }
                }
                AppCommand::SelectTile(tid) => {
                    selected_tile = tid;
                    let summary = doc_shared
                        .read()
                        .ok()
                        .and_then(|g| g.as_ref().and_then(|d| tid.and_then(|id| d.summary(id))));
                    let _ = evt_tx.send(EditorEvent::SelectionChanged { summary });
                }
                AppCommand::FocusTile(tid) => {
                    if let Ok(g) = doc_shared.read() {
                        if let Some(doc) = g.as_ref() {
                            let pose = doc.compute_focus_pose(tid.or(selected_tile));
                            camera.focus_on(pose);
                        }
                    }
                }
                AppCommand::PickAt { x, y } => {
                    let (vw, vh) = renderer
                        .as_ref()
                        .map(|r| (r.viewport_size().0 as f64, r.viewport_size().1 as f64))
                        .unwrap_or((1280.0, 720.0));
                    let (ray_o, ray_d) = camera.screen_to_ray(x, y, vw, vh);
                    let mut picked: Option<u64> = None;
                    if let Ok(g) = doc_shared.read() {
                        if let Some(doc) = g.as_ref() {
                            let candidates: Vec<u64> = if !last_render_tiles.is_empty() {
                                last_render_tiles.clone()
                            } else {
                                doc.all_nodes().iter().map(|n| n.id).collect()
                            };
                            if let Some(hit) = doc.pick_ray(ray_o, ray_d, &candidates, &|tid| {
                                cache.has_non_empty_mesh(tid).cloned()
                            }) {
                                picked = Some(hit.tile_id);
                            }
                        }
                    }
                    selected_tile = picked;
                    let summary = doc_shared
                        .read()
                        .ok()
                        .and_then(|g| g.as_ref().and_then(|d| picked.and_then(|id| d.summary(id))));
                    let _ = evt_tx.send(EditorEvent::SelectionChanged { summary });
                }
                AppCommand::SetTileTrs { tile_id, trs } => {
                    let mut updated = None;
                    if let Ok(mut w) = doc_shared.write() {
                        if let Some(doc) = w.as_mut() {
                            let cmd = Box::new(SetTileTrsCommand {
                                tile_id,
                                old_trs: None,
                                new_trs: trs,
                            });
                            if let Err(e) = history.execute(cmd, doc) {
                                let _ = evt_tx.send(EditorEvent::Error {
                                    message: format!("变换编辑失败: {e:#}"),
                                });
                            } else {
                                updated = doc.summary(tile_id);
                            }
                        }
                    }
                    if let Some(s) = updated {
                        let _ = evt_tx.send(EditorEvent::TileUpdated { summary: s });
                    }
                }
                AppCommand::SetTileVisible { tile_id, visible } => {
                    let mut updated = None;
                    if let Ok(mut w) = doc_shared.write() {
                        if let Some(doc) = w.as_mut() {
                            let cmd = Box::new(SetTileVisibleCommand {
                                tile_id,
                                old_visible: None,
                                new_visible: visible,
                            });
                            if let Err(e) = history.execute(cmd, doc) {
                                let _ = evt_tx.send(EditorEvent::Error {
                                    message: format!("显隐切换失败: {e:#}"),
                                });
                            } else {
                                updated = doc.summary(tile_id);
                            }
                        }
                    }
                    if let Some(s) = updated {
                        let _ = evt_tx.send(EditorEvent::TileUpdated { summary: s });
                    }
                }
                AppCommand::Undo => {
                    let mut updated = None;
                    if let Ok(mut w) = doc_shared.write() {
                        if let Some(doc) = w.as_mut() {
                            if let Ok(Some(tid)) = history.undo(doc) {
                                updated = doc.summary(tid);
                            }
                        }
                    }
                    if let Some(s) = updated {
                        let _ = evt_tx.send(EditorEvent::TileUpdated { summary: s });
                    }
                }
                AppCommand::Redo => {
                    let mut updated = None;
                    if let Ok(mut w) = doc_shared.write() {
                        if let Some(doc) = w.as_mut() {
                            if let Ok(Some(tid)) = history.redo(doc) {
                                updated = doc.summary(tid);
                            }
                        }
                    }
                    if let Some(s) = updated {
                        let _ = evt_tx.send(EditorEvent::TileUpdated { summary: s });
                    }
                }
            }
        }

        // 2. 回收异步加载完成的内容与外部子 tileset
        let current_gen = loader.current_generation();
        for item in loader.drain_completed(32) {
            match item {
                LoadCompleted::Content {
                    tile_id,
                    generation,
                    result,
                } => {
                    if generation != current_gen {
                        continue;
                    }
                    match result {
                        Ok(mesh) => {
                            cache.set_ready(tile_id, mesh, frame_no);
                            if let Some(r) = renderer.as_mut() {
                                if let Some(arc_mesh) = cache.has_non_empty_mesh(tile_id) {
                                    r.upload_tile(tile_id, arc_mesh, frame_no);
                                }
                            }
                        }
                        Err(e) => {
                            cache.set_failed(tile_id, format!("{e:#}"), frame_no);
                            push_log(&evt_tx, "warn", format!("瓦片 {tile_id} 解码失败: {e:#}"));
                        }
                    }
                }
                LoadCompleted::ExternalTileset {
                    parent_tile_id,
                    path,
                    generation,
                    result,
                } => {
                    external_loading.remove(&parent_tile_id);
                    if generation != current_gen {
                        continue;
                    }
                    match result {
                        Ok(ast) => {
                            if let Ok(mut w) = doc_shared.write() {
                                if let Some(doc) = w.as_mut() {
                                    if let Err(e) =
                                        doc.splice_external_tileset(parent_tile_id, path, ast)
                                    {
                                        push_log(&evt_tx, "warn", format!("外部子树合并失败: {e:#}"));
                                    } else {
                                        let _ = evt_tx.send(EditorEvent::ExternalTilesetMerged {
                                            parent_id: parent_tile_id,
                                        });
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            push_log(&evt_tx, "warn", format!("读取外部 tileset 失败: {e:#}"));
                        }
                    }
                }
            }
        }

        // 3. 执行视锥裁剪 + SSE LOD 调度 + LRU 淘汰
        let (vw, vh) = renderer
            .as_ref()
            .map(|r| (r.viewport_size().0 as f64, r.viewport_size().1 as f64))
            .unwrap_or((1280.0, 720.0));
        let aspect = (vw / vh.max(1.0)).max(0.01);
        let lod_params = LodParams {
            camera_pos: camera.eye_world(),
            frustum_planes_rel_cam: camera.frustum_planes_rel_cam(aspect),
            viewport_height: vh,
            fov_y_rad: camera.fov_y_rad,
            max_sse,
            max_requests_per_frame: 24,
            frame_number: frame_no,
        };

        let mut total_nodes = 0usize;
        let mut visited_nodes = 0usize;
        let mut pending_content_jobs: Vec<(u64, Vec<std::path::PathBuf>)> = Vec::new();
        let mut pending_ext_jobs: Vec<(u64, std::path::PathBuf)> = Vec::new();

        if let Ok(g) = doc_shared.read() {
            if let Some(doc) = g.as_ref() {
                total_nodes = doc.all_nodes().len();
                let lod = evaluate_lod(doc, &mut cache, &lod_params, &external_loading);
                visited_nodes = lod.visible_nodes_visited;
                last_render_tiles = lod.render_tiles;

                for tid in lod.load_content {
                    if let Some(node) = doc.node(tid) {
                        if !node.content_paths.is_empty() {
                            pending_content_jobs.push((tid, node.content_paths.clone()));
                        }
                    }
                }
                for tid in lod.load_external {
                    if let Some(node) = doc.node(tid) {
                        if let Some(ext) = &node.external_tileset_path {
                            pending_ext_jobs.push((tid, ext.clone()));
                        }
                    }
                }
            } else {
                last_render_tiles.clear();
            }
        }

        for (tid, paths) in pending_content_jobs {
            let cancel = cache.mark_loading(tid, frame_no);
            loader.submit_content(tid, paths, cancel);
        }
        for (tid, path) in pending_ext_jobs {
            if external_loading.insert(tid) {
                loader.submit_external_tileset(tid, path, Arc::new(AtomicBool::new(false)));
            }
        }

        let mut pinned: HashSet<u64> = last_render_tiles.iter().copied().collect();
        if let Some(sel) = selected_tile {
            pinned.insert(sel);
        }

        for evicted_id in cache.evict_over_budget(&pinned) {
            if let Some(r) = renderer.as_mut() {
                r.evict_tile(evicted_id);
            }
        }
        if let Some(r) = renderer.as_mut() {
            r.evict_gpu_over_budget(&pinned);
            // 确保在 render_tiles 中已处于 CPU Ready 但尚未传至 GPU 的节点立即可绘制
            for &tid in &last_render_tiles {
                if !r.has_gpu_tile(tid) {
                    if let Some(TileContentState::Ready(mesh)) = cache.state(tid) {
                        r.upload_tile(tid, mesh, frame_no);
                    }
                }
            }
        }

        // 4. 提交 wgpu 渲染帧
        let mut draw_calls = 0;
        let mut rendered_tiles = 0;
        let mut gpu_mb = 0.0;
        if let Some(r) = renderer.as_mut() {
            let doc_guard = doc_shared.read().ok();
            let doc_ref = doc_guard.as_ref().and_then(|g| g.as_ref());
            match r.render(
                &camera,
                doc_ref,
                &last_render_tiles,
                selected_tile,
                show_all_bboxes,
                frame_no,
            ) {
                Ok(stats) => {
                    draw_calls = stats.draw_calls;
                    rendered_tiles = stats.rendered_tiles;
                    gpu_mb = stats.gpu_memory_mb;
                }
                Err(e) => {
                    push_log(&evt_tx, "error", format!("渲染异常: {e:#}"));
                }
            }
        }

        frame_no += 1;
        frames_since_stats += 1;
        if last_stats_time.elapsed() >= Duration::from_millis(500) {
            let secs = last_stats_time.elapsed().as_secs_f32().max(0.001);
            let _ = evt_tx.send(EditorEvent::Stats {
                fps: frames_since_stats as f32 / secs,
                draw_calls,
                rendered_tiles,
                gpu_mb,
                pending_io: loader.pending_count(),
                total_nodes,
                vw: vw as u32,
                vh: vh as u32,
                visited: visited_nodes,
            });
            frames_since_stats = 0;
            last_stats_time = Instant::now();
        }

        let elapsed = frame_start.elapsed();
        let target = Duration::from_millis(16);
        if elapsed < target {
            thread::sleep(target - elapsed);
        }
    }
}
