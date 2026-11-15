use glam::DVec3;
use tiles_core::decode_pnts;

/// 构造最小 pnts 字节流（featureTableJSON 以空格填充到 8 字节对齐, 与真实产物一致）
fn build_pnts(ft_json: &str, ft_bin: &[u8]) -> Vec<u8> {
    let mut ft_json_bytes = ft_json.as_bytes().to_vec();
    while ft_json_bytes.len() % 8 != 0 {
        ft_json_bytes.push(b' ');
    }
    let total = 28 + ft_json_bytes.len() + ft_bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"pnts");
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(ft_json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&(ft_bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // batchTableJSON
    out.extend_from_slice(&0u32.to_le_bytes()); // batchTableBinary
    out.extend_from_slice(&ft_json_bytes);
    out.extend_from_slice(ft_bin);
    out
}

#[test]
fn pnts_position_rgb_rtc_center_decode() {
    let positions: [[f32; 3]; 3] = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]];
    let rgbs: [[u8; 3]; 3] = [[10, 20, 30], [40, 50, 60], [70, 80, 90]];

    let mut ft_bin = Vec::new();
    for p in &positions {
        for v in p {
            ft_bin.extend_from_slice(&v.to_le_bytes());
        }
    }
    let rgb_offset = ft_bin.len();
    for c in &rgbs {
        ft_bin.extend_from_slice(c);
    }

    let bytes = build_pnts(
        r#"{"POINTS_LENGTH":3,"POSITION":{"byteOffset":0},"RGB":{"byteOffset":36},"RTC_CENTER":[-1601667.0,5275274.0,3197174.0]}"#,
        &ft_bin,
    );
    let cloud = decode_pnts(&bytes).expect("decode pnts");

    assert_eq!(cloud.instances.len(), 3);
    assert_eq!(cloud.instances[0].pos, [1.0, 2.0, 3.0]);
    assert_eq!(cloud.instances[1].pos, [4.0, 5.0, 6.0]);
    assert_eq!(cloud.instances[1].color, [40, 50, 60, 255]);
    assert_eq!(cloud.instances[2].color, [70, 80, 90, 255]);
    // RTC_CENTER 折入平移矩阵
    assert_eq!(
        cloud.local_to_tile.w_axis.truncate(),
        DVec3::new(-1601667.0, 5275274.0, 3197174.0)
    );
    // PointInstance 为 16 字节对齐布局
    assert_eq!(std::mem::size_of::<tiles_core::PointInstance>(), 16);
    assert_eq!(cloud.approx_cpu_bytes, 3 * 16);
}

#[test]
fn pnts_quantized_positions_decode() {
    // POSITION = OFFSET + Q * SCALE / 65535; Q=65535, SCALE=2, OFFSET=10 -> 12
    let ft_bin: Vec<u8> = [65535u16, 65535, 65535]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let bytes = build_pnts(
        r#"{"POINTS_LENGTH":1,"POSITION_QUANTIZED":{"byteOffset":0},"QUANTIZED_VOLUME_OFFSET":[10.0,0.0,0.0],"QUANTIZED_VOLUME_SCALE":[2.0,2.0,2.0]}"#,
        &ft_bin,
    );
    let cloud = decode_pnts(&bytes).expect("decode quantized pnts");
    assert_eq!(cloud.instances.len(), 1);
    assert_eq!(cloud.instances[0].pos, [12.0, 2.0, 2.0]);
    // 无颜色定义 -> 默认白色
    assert_eq!(cloud.instances[0].color, [255, 255, 255, 255]);
}

#[test]
fn pnts_constant_rgba_and_rgb565() {
    let positions: Vec<u8> = [[0.0f32, 0.0, 0.0], [1.0, 1.0, 1.0]]
        .iter()
        .flat_map(|p| p.iter().flat_map(|v| v.to_le_bytes()))
        .collect();
    let bytes = build_pnts(
        r#"{"POINTS_LENGTH":2,"POSITION":{"byteOffset":0},"CONSTANT_RGBA":[255,0,0,128]}"#,
        &positions,
    );
    let cloud = decode_pnts(&bytes).expect("decode constant rgba pnts");
    assert_eq!(cloud.instances[0].color, [255, 0, 0, 128]);
    assert_eq!(cloud.instances[1].color, [255, 0, 0, 128]);

    // RGB565: v = 0xFFFF -> (255,255,255); POSITION 段在前, RGB565 跟随其后
    let mut rgb565_bin: Vec<u8> = [0.0f32, 0.0, 0.0]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let rgb565_offset = rgb565_bin.len();
    rgb565_bin.extend_from_slice(&0xFFFFu16.to_le_bytes());
    let bytes = build_pnts(
        &format!(
            r#"{{"POINTS_LENGTH":1,"POSITION":{{"byteOffset":0}},"RGB565":{{"byteOffset":{rgb565_offset}}}}}"#
        ),
        &rgb565_bin,
    );
    let cloud = decode_pnts(&bytes).expect("decode rgb565 pnts");
    assert_eq!(cloud.instances[0].color, [255, 255, 255, 255]);
}

#[test]
fn pnts_truncated_header_rejected() {
    let err = decode_pnts(b"pnt").unwrap_err().to_string();
    assert!(err.contains("Truncated pnts header"), "got: {err}");

    let mut bad = build_pnts(
        r#"{"POINTS_LENGTH":1,"POSITION":{"byteOffset":0}}"#,
        &[0u8; 12],
    );
    bad.truncate(20); // 截掉 JSON 段
    assert!(decode_pnts(&bad).is_err());
}

#[test]
fn pnts_draco_compressed_reported_clearly() {
    let bytes = build_pnts(r#"{"POINTS_LENGTH":1,"3DR_compression":{},"POSITION_QUANTIZED":{"byteOffset":0},"QUANTIZED_VOLUME_OFFSET":[0,0,0],"QUANTIZED_VOLUME_SCALE":[1,1,1]}"#, &[]);
    let err = decode_pnts(&bytes).unwrap_err().to_string();
    assert!(
        err.contains("DRACO-compressed point cloud"),
        "got: {err}"
    );
}

#[test]
fn pnts_missing_position_rejected() {
    let bytes = build_pnts(r#"{"POINTS_LENGTH":1}"#, &[]);
    let err = decode_pnts(&bytes).unwrap_err().to_string();
    assert!(
        err.contains("neither POSITION nor POSITION_QUANTIZED"),
        "got: {err}"
    );
}
