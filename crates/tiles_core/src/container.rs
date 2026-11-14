use anyhow::{bail, Context, Result};
use glam::DVec3;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileContainerKind {
    Glb,
    B3dm,
    Cmpt,
    I3dm,
    Pnts,
}

#[derive(Debug, Clone)]
pub struct ExtractedGlb<'a> {
    pub glb_bytes: &'a [u8],
    pub rtc_center: DVec3,
    pub batch_length: u32,
}

#[derive(Debug, Deserialize)]
struct B3dmFeatureTableJson {
    #[serde(rename = "RTC_CENTER")]
    rtc_center: Option<[f64; 3]>,
    #[serde(rename = "BATCH_LENGTH")]
    batch_length: Option<u32>,
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<usize> {
    let slice = bytes
        .get(offset..offset + 4)
        .context("Unexpected EOF reading tile header")?;
    Ok(u32::from_le_bytes(slice.try_into().unwrap()) as usize)
}

pub fn detect_container_kind(bytes: &[u8]) -> Result<TileContainerKind> {
    if bytes.len() < 4 {
        bail!("Tile content shorter than 4 bytes");
    }
    match &bytes[0..4] {
        b"glTF" => Ok(TileContainerKind::Glb),
        b"b3dm" => Ok(TileContainerKind::B3dm),
        b"cmpt" => Ok(TileContainerKind::Cmpt),
        b"i3dm" => Ok(TileContainerKind::I3dm),
        b"pnts" => Ok(TileContainerKind::Pnts),
        other => bail!("Unsupported 3D Tiles binary magic: {:?}", other),
    }
}

/// 递归提取单个或多个内嵌 GLB（支持 glb / b3dm / cmpt；对 i3dm/pnts 明确报告扩展类型占位错误）
pub fn extract_glb_payloads(bytes: &[u8]) -> Result<Vec<ExtractedGlb<'_>>> {
    let mut out = Vec::new();
    extract_recursive(bytes, &mut out)?;
    Ok(out)
}

fn extract_recursive<'a>(bytes: &'a [u8], out: &mut Vec<ExtractedGlb<'a>>) -> Result<()> {
    match detect_container_kind(bytes)? {
        TileContainerKind::Glb => {
            out.push(ExtractedGlb {
                glb_bytes: bytes,
                rtc_center: DVec3::ZERO,
                batch_length: 0,
            });
            Ok(())
        }
        TileContainerKind::B3dm => {
            if bytes.len() < 28 {
                bail!("Truncated b3dm header");
            }
            let version = read_u32_le(bytes, 4)?;
            let byte_len = read_u32_le(bytes, 8)?;
            let ft_json_len = read_u32_le(bytes, 12)?;
            let ft_bin_len = read_u32_le(bytes, 16)?;
            let bt_json_len = read_u32_le(bytes, 20)?;
            let bt_bin_len = read_u32_le(bytes, 24)?;
            if version != 1 || byte_len > bytes.len() {
                bail!("Invalid b3dm header: version={version}, byte_len={byte_len}");
            }

            let ft_json_start = 28;
            let ft_json_end = ft_json_start + ft_json_len;
            let glb_start = ft_json_end + ft_bin_len + bt_json_len + bt_bin_len;
            if glb_start > byte_len {
                bail!("Invalid b3dm payload offsets");
            }

            let (rtc_center, batch_length) = if ft_json_len > 0 {
                let ft: B3dmFeatureTableJson =
                    serde_json::from_slice(&bytes[ft_json_start..ft_json_end])
                        .context("Failed to parse b3dm featureTable JSON")?;
                (
                    ft.rtc_center.map(DVec3::from_array).unwrap_or(DVec3::ZERO),
                    ft.batch_length.unwrap_or(0),
                )
            } else {
                (DVec3::ZERO, 0)
            };

            if glb_start < byte_len {
                out.push(ExtractedGlb {
                    glb_bytes: &bytes[glb_start..byte_len],
                    rtc_center,
                    batch_length,
                });
            }
            Ok(())
        }
        TileContainerKind::Cmpt => {
            if bytes.len() < 16 {
                bail!("Truncated cmpt header");
            }
            let version = read_u32_le(bytes, 4)?;
            let byte_len = read_u32_le(bytes, 8)?;
            let tiles_len = read_u32_le(bytes, 12)?;
            if version != 1 || byte_len > bytes.len() {
                bail!("Invalid cmpt header");
            }
            let mut cursor = 16;
            for _ in 0..tiles_len {
                if cursor + 12 > byte_len {
                    bail!("Truncated inner tile inside cmpt");
                }
                let inner_len = read_u32_le(bytes, cursor + 8)?;
                if inner_len < 12 || cursor + inner_len > byte_len {
                    bail!("Invalid inner tile byteLength inside cmpt: {inner_len}");
                }
                extract_recursive(&bytes[cursor..cursor + inner_len], out)?;
                cursor += inner_len;
            }
            Ok(())
        }
        TileContainerKind::I3dm => {
            bail!("i3dm (Instanced 3D Model) container is scheduled for Phase 2")
        }
        TileContainerKind::Pnts => {
            bail!("pnts (Point Cloud) container is scheduled for Phase 2")
        }
    }
}
