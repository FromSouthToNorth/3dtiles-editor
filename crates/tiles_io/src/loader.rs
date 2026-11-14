use anyhow::{Context, Result};
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tiles_core::{extract_glb_payloads, load_glb_or_gltf, DecodedMesh, TilesetJson};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::Semaphore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadJobKind {
    Content,
    ExternalTileset,
}

pub enum LoadCompleted {
    Content {
        tile_id: u64,
        generation: u64,
        result: Result<DecodedMesh>,
    },
    ExternalTileset {
        parent_tile_id: u64,
        path: PathBuf,
        generation: u64,
        result: Result<TilesetJson>,
    },
}

pub struct AsyncTileLoader {
    runtime: Runtime,
    semaphore: Arc<Semaphore>,
    generation: Arc<AtomicU64>,
    tx: Sender<LoadCompleted>,
    rx: Receiver<LoadCompleted>,
    pending_count: Arc<AtomicU64>,
}

impl AsyncTileLoader {
    pub fn new(max_concurrent_jobs: usize) -> Result<Self> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(4)
            .thread_name("tiles-io-worker")
            .enable_all()
            .build()
            .context("Failed to initialize Tokio IO runtime")?;
        let (tx, rx) = unbounded();
        Ok(Self {
            runtime,
            semaphore: Arc::new(Semaphore::new(max_concurrent_jobs.max(1))),
            generation: Arc::new(AtomicU64::new(1)),
            tx,
            rx,
            pending_count: Arc::new(AtomicU64::new(0)),
        })
    }

    pub fn bump_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    pub fn pending_count(&self) -> u64 {
        self.pending_count.load(Ordering::Relaxed)
    }

    pub fn submit_content(&self, tile_id: u64, paths: Vec<PathBuf>, cancel: Arc<AtomicBool>) {
        let gen = self.current_generation();
        let gen_ref = self.generation.clone();
        let sem = self.semaphore.clone();
        let tx = self.tx.clone();
        let pending = self.pending_count.clone();
        pending.fetch_add(1, Ordering::Relaxed);

        self.runtime.spawn(async move {
            let _permit = sem.acquire_owned().await;
            if cancel.load(Ordering::Relaxed) || gen_ref.load(Ordering::Relaxed) != gen {
                pending.fetch_sub(1, Ordering::Relaxed);
                return;
            }
            let res = tokio::task::spawn_blocking(move || decode_tile_paths(&paths)).await;
            let result = match res {
                Ok(r) => r,
                Err(e) => Err(anyhow::anyhow!("IO task join error: {e}")),
            };
            pending.fetch_sub(1, Ordering::Relaxed);
            if !cancel.load(Ordering::Relaxed) && gen_ref.load(Ordering::Relaxed) == gen {
                let _ = tx.send(LoadCompleted::Content {
                    tile_id,
                    generation: gen,
                    result,
                });
            }
        });
    }

    pub fn submit_external_tileset(&self, parent_tile_id: u64, path: PathBuf, cancel: Arc<AtomicBool>) {
        let gen = self.current_generation();
        let gen_ref = self.generation.clone();
        let sem = self.semaphore.clone();
        let tx = self.tx.clone();
        let pending = self.pending_count.clone();
        pending.fetch_add(1, Ordering::Relaxed);

        self.runtime.spawn(async move {
            let _permit = sem.acquire_owned().await;
            if cancel.load(Ordering::Relaxed) || gen_ref.load(Ordering::Relaxed) != gen {
                pending.fetch_sub(1, Ordering::Relaxed);
                return;
            }
            let p_clone = path.clone();
            let res = tokio::task::spawn_blocking(move || -> Result<TilesetJson> {
                let text = std::fs::read_to_string(&p_clone)
                    .with_context(|| format!("Failed to read external tileset {}", p_clone.display()))?;
                let ast: TilesetJson = serde_json::from_str(&text)
                    .with_context(|| format!("Invalid external tileset {}", p_clone.display()))?;
                Ok(ast)
            })
            .await;
            let result = match res {
                Ok(r) => r,
                Err(e) => Err(anyhow::anyhow!("External tileset join error: {e}")),
            };
            pending.fetch_sub(1, Ordering::Relaxed);
            if !cancel.load(Ordering::Relaxed) && gen_ref.load(Ordering::Relaxed) == gen {
                let _ = tx.send(LoadCompleted::ExternalTileset {
                    parent_tile_id,
                    path,
                    generation: gen,
                    result,
                });
            }
        });
    }

    pub fn drain_completed(&self, max_items: usize) -> Vec<LoadCompleted> {
        let mut out = Vec::new();
        while out.len() < max_items {
            match self.rx.try_recv() {
                Ok(item) => out.push(item),
                Err(_) => break,
            }
        }
        out
    }
}

fn decode_tile_paths(paths: &[PathBuf]) -> Result<DecodedMesh> {
    let mut merged = DecodedMesh::empty();
    for path in paths {
        let bytes = std::fs::read(path)
            .with_context(|| format!("Failed to read tile content {}", path.display()))?;
        let base_dir = path.parent();
        let payloads = extract_glb_payloads(&bytes)?;
        for payload in payloads {
            let mut decoded = load_glb_or_gltf(payload.glb_bytes, base_dir, payload.rtc_center)?;
            let tex_offset = merged.textures.len();
            merged.textures.append(&mut decoded.textures);
            for mut prim in decoded.primitives {
                if let Some(idx) = prim.base_color_texture.as_mut() {
                    *idx += tex_offset;
                }
                merged.primitives.push(prim);
            }
            merged.approx_cpu_bytes += decoded.approx_cpu_bytes;
        }
    }
    Ok(merged)
}
