use anyhow::{bail, Context, Result};
use bytemuck::{Pod, Zeroable};
use glam::{DMat4, DVec3};
use serde::Deserialize;

/// pnts 点实例: 位置(f32x3) + RGBA8 颜色, 共 16 字节,
/// 与渲染端实例步进布局 (Float32x3 @0 + Unorm8x4 @12) 直接对齐
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PointInstance {
    pub pos: [f32; 3],
    pub color: [u8; 4],
}

/// pnts 解码产物: 点集 + tile 局部变换 (RTC_CENTER 已折入平移, 无需 Y-up 旋转 —— 点云坐标按规范即 Z-up tile 空间)
#[derive(Debug, Clone)]
pub struct DecodedPointCloud {
    pub instances: Vec<PointInstance>,
    pub local_to_tile: DMat4,
    pub approx_cpu_bytes: usize,
}

#[derive(Debug, Deserialize)]
struct BinaryReference {
    #[serde(rename = "byteOffset")]
    byte_offset: u32,
}

#[derive(Debug, Deserialize)]
struct PntsFeatureTableJson {
    #[serde(rename = "POINTS_LENGTH")]
    points_length: u32,
    #[serde(rename = "POSITION")]
    position: Option<BinaryReference>,
    #[serde(rename = "POSITION_QUANTIZED")]
    position_quantized: Option<BinaryReference>,
    #[serde(rename = "QUANTIZED_VOLUME_OFFSET")]
    quantized_volume_offset: Option<[f64; 3]>,
    #[serde(rename = "QUANTIZED_VOLUME_SCALE")]
    quantized_volume_scale: Option<[f64; 3]>,
    #[serde(rename = "RGB")]
    rgb: Option<BinaryReference>,
    #[serde(rename = "RGBA")]
    rgba: Option<BinaryReference>,
    #[serde(rename = "RGB565")]
    rgb565: Option<BinaryReference>,
    #[serde(rename = "CONSTANT_RGBA")]
    constant_rgba: Option<[u8; 4]>,
    #[serde(rename = "RTC_CENTER")]
    rtc_center: Option<[f64; 3]>,
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<usize> {
    let slice = bytes
        .get(offset..offset + 4)
        .context("Unexpected EOF reading pnts header")?;
    Ok(u32::from_le_bytes(slice.try_into().unwrap()) as usize)
}

/// 解析 3D Tiles legacy 点云瓦片 (.pnts, magic "pnts")
///
/// 支持: POSITION / POSITION_QUANTIZED(+QUANTIZED_VOLUME_OFFSET/SCALE)、RGB / RGBA / RGB565 / CONSTANT_RGBA、RTC_CENTER。
/// 不支持: DRACO 压缩点云 (带明确报错)、Batch Table (忽略)。
pub fn decode_pnts(bytes: &[u8]) -> Result<DecodedPointCloud> {
    if bytes.len() < 28 {
        bail!("Truncated pnts header");
    }
    if &bytes[0..4] != b"pnts" {
        bail!("Invalid pnts magic: {:?}", &bytes[0..4]);
    }
    let version = read_u32_le(bytes, 4)?;
    let byte_len = read_u32_le(bytes, 8)?;
    let ft_json_len = read_u32_le(bytes, 12)?;
    let ft_bin_len = read_u32_le(bytes, 16)?;
    let _bt_json_len = read_u32_le(bytes, 20)?;
    let _bt_bin_len = read_u32_le(bytes, 24)?;
    if version != 1 {
        bail!("Unsupported pnts version: {version}");
    }
    let byte_len = byte_len.min(bytes.len());
    if 28 + ft_json_len + ft_bin_len > byte_len {
        bail!("Invalid pnts payload offsets");
    }

    let ft = if ft_json_len > 0 {
        let raw = &bytes[28..28 + ft_json_len];
        if raw.windows(3).any(|w| w == b"3DR") {
            bail!("Unsupported pnts: DRACO-compressed point cloud (3DTILES_draco_point_compression) is not supported yet");
        }
        serde_json::from_slice::<PntsFeatureTableJson>(raw)
            .context("Failed to parse pnts featureTable JSON")?
    } else {
        bail!("pnts featureTable JSON is empty (missing POINTS_LENGTH)");
    };

    let points_len = ft.points_length as usize;
    if points_len == 0 {
        return Ok(DecodedPointCloud {
            instances: Vec::new(),
            local_to_tile: DMat4::IDENTITY,
            approx_cpu_bytes: 0,
        });
    }

    // Feature Table 二进制段起点 (byteOffset 相对该段起始)
    let bin_start = 28 + ft_json_len;
    let bin = &bytes[bin_start..(bin_start + ft_bin_len).min(byte_len)];
    let read_slice = |offset: usize, size: usize, what: &str| -> Result<&[u8]> {
        let end = size
            .checked_mul(points_len)
            .and_then(|n| offset.checked_add(n))
            .context("pnts feature table offset overflow")?;
        bin.get(offset..end)
            .with_context(|| format!("Truncated pnts feature table binary: {what}"))
    };

    // ---- 位置 ----
    let positions: Vec<[f32; 3]> = if let Some(pos) = ft.position.as_ref() {
        let raw = read_slice(pos.byte_offset as usize, 12, "POSITION")?;
        raw.chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect::<Vec<f32>>()
            .chunks_exact(3)
            .map(|c| [c[0], c[1], c[2]])
            .collect()
    } else if let Some(pos) = ft.position_quantized.as_ref() {
        let (Some(vol_offset), Some(vol_scale)) = (ft.quantized_volume_offset, ft.quantized_volume_scale)
        else {
            bail!("POSITION_QUANTIZED requires QUANTIZED_VOLUME_OFFSET and QUANTIZED_VOLUME_SCALE");
        };
        let raw = read_slice(pos.byte_offset as usize, 6, "POSITION_QUANTIZED")?;
        raw.chunks_exact(2)
            .map(|c| u16::from_le_bytes(c.try_into().unwrap()) as f64)
            .collect::<Vec<f64>>()
            .chunks_exact(3)
            .map(|q| {
                [
                    (vol_offset[0] + q[0] * vol_scale[0] / 65535.0) as f32,
                    (vol_offset[1] + q[1] * vol_scale[1] / 65535.0) as f32,
                    (vol_offset[2] + q[2] * vol_scale[2] / 65535.0) as f32,
                ]
            })
            .collect()
    } else {
        bail!("pnts feature table has neither POSITION nor POSITION_QUANTIZED (possibly DRACO-compressed)");
    };
    if positions.len() != points_len {
        bail!("pnts POSITION element count {} != POINTS_LENGTH {points_len}", positions.len());
    }

    // ---- 颜色 ----
    let constant = ft.constant_rgba.unwrap_or([255, 255, 255, 255]);
    let colors: Vec<[u8; 4]> = if let Some(rgba) = ft.rgba.as_ref() {
        let raw = read_slice(rgba.byte_offset as usize, 4, "RGBA")?;
        raw.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect()
    } else if let Some(rgb) = ft.rgb.as_ref() {
        let raw = read_slice(rgb.byte_offset as usize, 3, "RGB")?;
        raw.chunks_exact(3).map(|c| [c[0], c[1], c[2], 255]).collect()
    } else if let Some(rgb565) = ft.rgb565.as_ref() {
        let raw = read_slice(rgb565.byte_offset as usize, 2, "RGB565")?;
        raw.chunks_exact(2)
            .map(|c| {
                let v = u16::from_le_bytes(c.try_into().unwrap());
                [
                    (((v >> 11) & 0x1F) as f32 * 255.0 / 31.0) as u8,
                    (((v >> 5) & 0x3F) as f32 * 255.0 / 63.0) as u8,
                    ((v & 0x1F) as f32 * 255.0 / 31.0) as u8,
                    255,
                ]
            })
            .collect()
    } else {
        vec![constant; points_len]
    };
    if colors.len() != points_len {
        bail!("pnts color element count {} != POINTS_LENGTH {points_len}", colors.len());
    }

    let instances: Vec<PointInstance> = positions
        .into_iter()
        .zip(colors)
        .map(|(pos, color)| PointInstance { pos, color })
        .collect();

    let rtc = ft
        .rtc_center
        .map(|c| DMat4::from_translation(DVec3::from_array(c)))
        .unwrap_or(DMat4::IDENTITY);
    Ok(DecodedPointCloud {
        approx_cpu_bytes: instances.len() * std::mem::size_of::<PointInstance>(),
        instances,
        local_to_tile: rtc,
    })
}
