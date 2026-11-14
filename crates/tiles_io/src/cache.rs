use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tiles_core::DecodedMesh;

#[derive(Debug, Clone)]
pub enum TileContentState {
    Unloaded,
    Loading { cancel: Arc<AtomicBool> },
    Ready(Arc<DecodedMesh>),
    ReadyEmpty,
    Failed(String),
}

struct Entry {
    state: TileContentState,
    last_used_frame: u64,
    cpu_bytes: usize,
}

pub struct TileCache {
    entries: HashMap<u64, Entry>,
    total_cpu_bytes: usize,
    budget_bytes: usize,
}

impl TileCache {
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            total_cpu_bytes: 0,
            budget_bytes,
        }
    }

    pub fn clear(&mut self) {
        for entry in self.entries.values() {
            if let TileContentState::Loading { cancel } = &entry.state {
                cancel.store(true, Ordering::Relaxed);
            }
        }
        self.entries.clear();
        self.total_cpu_bytes = 0;
    }

    pub fn state(&self, tile_id: u64) -> Option<&TileContentState> {
        self.entries.get(&tile_id).map(|e| &e.state)
    }

    pub fn is_renderable_ready(&self, tile_id: u64) -> bool {
        matches!(
            self.state(tile_id),
            Some(TileContentState::Ready(_) | TileContentState::ReadyEmpty)
        )
    }

    pub fn has_non_empty_mesh(&self, tile_id: u64) -> Option<&Arc<DecodedMesh>> {
        match self.state(tile_id) {
            Some(TileContentState::Ready(m)) => Some(m),
            _ => None,
        }
    }

    pub fn mark_loading(&mut self, tile_id: u64, frame: u64) -> Arc<AtomicBool> {
        if let Some(prev) = self.entries.get(&tile_id) {
            if let TileContentState::Loading { cancel } = &prev.state {
                return cancel.clone();
            }
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.entries.insert(
            tile_id,
            Entry {
                state: TileContentState::Loading {
                    cancel: cancel.clone(),
                },
                last_used_frame: frame,
                cpu_bytes: 0,
            },
        );
        cancel
    }

    pub fn cancel_loading(&mut self, tile_id: u64) {
        if let Some(entry) = self.entries.get(&tile_id) {
            if let TileContentState::Loading { cancel } = &entry.state {
                cancel.store(true, Ordering::Relaxed);
                self.entries.remove(&tile_id);
            }
        }
    }

    pub fn set_ready(&mut self, tile_id: u64, mesh: DecodedMesh, frame: u64) {
        if let Some(old) = self.entries.remove(&tile_id) {
            self.total_cpu_bytes = self.total_cpu_bytes.saturating_sub(old.cpu_bytes);
        }
        if mesh.is_empty() {
            self.entries.insert(
                tile_id,
                Entry {
                    state: TileContentState::ReadyEmpty,
                    last_used_frame: frame,
                    cpu_bytes: 0,
                },
            );
        } else {
            let bytes = mesh.approx_cpu_bytes;
            self.total_cpu_bytes += bytes;
            self.entries.insert(
                tile_id,
                Entry {
                    state: TileContentState::Ready(Arc::new(mesh)),
                    last_used_frame: frame,
                    cpu_bytes: bytes,
                },
            );
        }
    }

    pub fn set_failed(&mut self, tile_id: u64, err: String, frame: u64) {
        if let Some(old) = self.entries.remove(&tile_id) {
            self.total_cpu_bytes = self.total_cpu_bytes.saturating_sub(old.cpu_bytes);
        }
        self.entries.insert(
            tile_id,
            Entry {
                state: TileContentState::Failed(err),
                last_used_frame: frame,
                cpu_bytes: 0,
            },
        );
    }

    pub fn touch(&mut self, tile_id: u64, frame: u64) {
        if let Some(e) = self.entries.get_mut(&tile_id) {
            e.last_used_frame = frame;
        }
    }

    pub fn stats(&self) -> (usize, usize) {
        (self.entries.len(), self.total_cpu_bytes)
    }

    pub fn evict_over_budget(&mut self, pinned: &HashSet<u64>) -> Vec<u64> {
        if self.total_cpu_bytes <= self.budget_bytes {
            return Vec::new();
        }
        let mut candidates: Vec<(u64, u64)> = self
            .entries
            .iter()
            .filter_map(|(&id, e)| {
                if pinned.contains(&id) {
                    return None;
                }
                match e.state {
                    TileContentState::Ready(_) | TileContentState::Failed(_) => {
                        Some((id, e.last_used_frame))
                    }
                    _ => None,
                }
            })
            .collect();
        candidates.sort_by_key(|&(_, f)| f);

        let mut evicted = Vec::new();
        for (id, _) in candidates {
            if self.total_cpu_bytes <= self.budget_bytes {
                break;
            }
            if let Some(removed) = self.entries.remove(&id) {
                self.total_cpu_bytes = self.total_cpu_bytes.saturating_sub(removed.cpu_bytes);
                evicted.push(id);
            }
        }
        evicted
    }
}
