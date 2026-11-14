use anyhow::Result;
use std::path::PathBuf;
use tiles_core::{decompose_trs, TilesetDoc, Trs};

#[derive(Debug, Clone)]
pub enum AppCommand {
    OpenTileset(PathBuf),
    SaveTileset(Option<PathBuf>),
    Resize { width: u32, height: u32 },
    Orbit { dx: f64, dy: f64 },
    Pan { dx: f64, dy: f64 },
    Zoom { delta: f64 },
    PickAt { x: f64, y: f64 },
    SelectTile(Option<u64>),
    FocusTile(Option<u64>),
    SetMaxSse(f64),
    SetShowAllBBoxes(bool),
    SetTileTrs { tile_id: u64, trs: Trs },
    SetTileVisible { tile_id: u64, visible: bool },
    Undo,
    Redo,
    Shutdown,
}

pub trait EditCommand: Send {
    fn label(&self) -> &str;
    fn target_tile(&self) -> u64;
    fn apply(&mut self, doc: &mut TilesetDoc) -> Result<()>;
    fn revert(&mut self, doc: &mut TilesetDoc) -> Result<()>;
}

pub struct SetTileTrsCommand {
    pub tile_id: u64,
    pub old_trs: Option<Trs>,
    pub new_trs: Trs,
}

impl EditCommand for SetTileTrsCommand {
    fn label(&self) -> &str {
        "Transform Tile"
    }

    fn target_tile(&self) -> u64 {
        self.tile_id
    }

    fn apply(&mut self, doc: &mut TilesetDoc) -> Result<()> {
        if self.old_trs.is_none() {
            if let Some(node) = doc.node(self.tile_id) {
                self.old_trs = Some(decompose_trs(node.local_transform));
            }
        }
        doc.set_tile_trs(self.tile_id, self.new_trs)
    }

    fn revert(&mut self, doc: &mut TilesetDoc) -> Result<()> {
        if let Some(old) = self.old_trs {
            doc.set_tile_trs(self.tile_id, old)?;
        }
        Ok(())
    }
}

pub struct SetTileVisibleCommand {
    pub tile_id: u64,
    pub old_visible: Option<bool>,
    pub new_visible: bool,
}

impl EditCommand for SetTileVisibleCommand {
    fn label(&self) -> &str {
        "Toggle Visibility"
    }

    fn target_tile(&self) -> u64 {
        self.tile_id
    }

    fn apply(&mut self, doc: &mut TilesetDoc) -> Result<()> {
        if self.old_visible.is_none() {
            self.old_visible = doc.node(self.tile_id).map(|n| n.visible);
        }
        doc.set_tile_visible(self.tile_id, self.new_visible)
    }

    fn revert(&mut self, doc: &mut TilesetDoc) -> Result<()> {
        if let Some(old) = self.old_visible {
            doc.set_tile_visible(self.tile_id, old)?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct CommandHistory {
    undo_stack: Vec<Box<dyn EditCommand>>,
    redo_stack: Vec<Box<dyn EditCommand>>,
}

impl CommandHistory {
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn execute(&mut self, mut cmd: Box<dyn EditCommand>, doc: &mut TilesetDoc) -> Result<u64> {
        cmd.apply(doc)?;
        let tid = cmd.target_tile();
        self.undo_stack.push(cmd);
        self.redo_stack.clear();
        Ok(tid)
    }

    pub fn undo(&mut self, doc: &mut TilesetDoc) -> Result<Option<u64>> {
        let Some(mut cmd) = self.undo_stack.pop() else {
            return Ok(None);
        };
        cmd.revert(doc)?;
        let tid = cmd.target_tile();
        self.redo_stack.push(cmd);
        Ok(Some(tid))
    }

    pub fn redo(&mut self, doc: &mut TilesetDoc) -> Result<Option<u64>> {
        let Some(mut cmd) = self.redo_stack.pop() else {
            return Ok(None);
        };
        cmd.apply(doc)?;
        let tid = cmd.target_tile();
        self.undo_stack.push(cmd);
        Ok(Some(tid))
    }
}
