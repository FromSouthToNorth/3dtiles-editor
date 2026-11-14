use glam::{DMat4, DQuat, DVec3, EulerRot};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TilesetJson {
    pub asset: AssetMetadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometric_error: Option<f64>,
    pub root: TileJson,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions_used: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions_required: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetMetadata {
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tileset_version: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RefineMode {
    Add,
    Replace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BoundingVolumeJson {
    Box {
        #[serde(rename = "box")]
        b: [f64; 12],
    },
    Region {
        region: [f64; 6],
    },
    Sphere {
        sphere: [f64; 4],
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentJson {
    #[serde(alias = "url")]
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounding_volume: Option<BoundingVolumeJson>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TileJson {
    pub bounding_volume: BoundingVolumeJson,
    pub geometric_error: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refine: Option<RefineMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<[f64; 16]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<ContentJson>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contents: Vec<ContentJson>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TileJson>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl TileJson {
    pub fn content_items(&self) -> Vec<&ContentJson> {
        let mut out = Vec::new();
        if let Some(c) = &self.content {
            out.push(c);
        }
        for c in &self.contents {
            out.push(c);
        }
        out
    }

    pub fn local_matrix(&self) -> DMat4 {
        match self.transform {
            Some(arr) => DMat4::from_cols_array(&arr),
            None => DMat4::IDENTITY,
        }
    }

    pub fn is_editor_visible(&self) -> bool {
        self.extra
            .get("extras")
            .and_then(|v| v.as_object())
            .and_then(|e| e.get("3dtiles_editor"))
            .and_then(|v| v.as_object())
            .and_then(|ed| ed.get("hidden"))
            .and_then(|b| b.as_bool())
            .map(|hidden| !hidden)
            .unwrap_or(true)
    }

    pub fn set_editor_visible(&mut self, visible: bool) {
        if visible {
            let mut remove_extras = false;
            if let Some(Value::Object(extras)) = self.extra.get_mut("extras") {
                if let Some(Value::Object(ed)) = extras.get_mut("3dtiles_editor") {
                    ed.remove("hidden");
                    if ed.is_empty() {
                        extras.remove("3dtiles_editor");
                    }
                }
                if extras.is_empty() {
                    remove_extras = true;
                }
            }
            if remove_extras {
                self.extra.remove("extras");
            }
        } else {
            let extras = self
                .extra
                .entry("extras".to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if let Value::Object(map) = extras {
                let ed = map
                    .entry("3dtiles_editor".to_string())
                    .or_insert_with(|| Value::Object(Map::new()));
                if let Value::Object(ed_map) = ed {
                    ed_map.insert("hidden".to_string(), json!(true));
                }
            }
        }
    }

    pub fn set_transform_matrix(&mut self, mat: DMat4) {
        if mat.abs_diff_eq(DMat4::IDENTITY, 1e-12) {
            self.transform = None;
        } else {
            self.transform = Some(mat.to_cols_array());
        }
    }
}

pub fn resolve_tile_uri(base_dir: &Path, uri: &str) -> PathBuf {
    let clean = uri.split('?').next().unwrap_or(uri);
    let rel = clean.replace('\\', "/");
    base_dir.join(rel)
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Trs {
    pub translation: [f64; 3],
    pub rotation_deg: [f64; 3],
    pub scale: [f64; 3],
}

impl Default for Trs {
    fn default() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
            rotation_deg: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

pub fn compose_trs(trs: Trs) -> DMat4 {
    let t = DVec3::from_array(trs.translation);
    let q = DQuat::from_euler(
        EulerRot::XYZ,
        trs.rotation_deg[0].to_radians(),
        trs.rotation_deg[1].to_radians(),
        trs.rotation_deg[2].to_radians(),
    );
    let s = DVec3::from_array(trs.scale);
    DMat4::from_scale_rotation_translation(s, q, t)
}

pub fn decompose_trs(mat: DMat4) -> Trs {
    let (scale, rot, trans) = mat.to_scale_rotation_translation();
    let (rx, ry, rz) = rot.to_euler(EulerRot::XYZ);
    Trs {
        translation: trans.to_array(),
        rotation_deg: [rx.to_degrees(), ry.to_degrees(), rz.to_degrees()],
        scale: scale.to_array(),
    }
}
