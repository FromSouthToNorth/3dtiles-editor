use serde::Serialize;
use tiles_core::TileSummaryDto;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EditorEvent {
    Log {
        level: String,
        message: String,
    },
    Progress {
        active: bool,
        progress: f32,
        text: String,
    },
    TilesetOpened {
        root_id: u64,
        path: String,
        node_count: usize,
    },
    ExternalTilesetMerged {
        parent_id: u64,
    },
    SelectionChanged {
        summary: Option<TileSummaryDto>,
    },
    TileUpdated {
        summary: TileSummaryDto,
    },
    Stats {
        fps: f32,
        draw_calls: u32,
        rendered_tiles: u32,
        gpu_mb: f32,
        pending_io: u64,
        total_nodes: usize,
        vw: u32,
        vh: u32,
        visited: usize,
    },
    TilesetSaved {
        files: Vec<String>,
    },
    Error {
        message: String,
    },
}
