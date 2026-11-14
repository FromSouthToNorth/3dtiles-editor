use glam::DVec3;
use std::path::PathBuf;
use tiles_core::container::extract_glb_payloads;
use tiles_core::schema::Trs;
use tiles_core::TilesetDoc;

#[test]
fn b3dm_container_unpack_preserves_rtc_center_and_glb() {
    let ft_json = br#"{"BATCH_LENGTH":2,"RTC_CENTER":[100.0,200.0,300.0]}   "#; // 4-byte aligned
    let fake_glb = b"glTF\x02\x00\x00\x00\x0c\x00\x00\x00";
    let total_len = 28 + ft_json.len() + fake_glb.len();

    let mut b3dm = Vec::with_capacity(total_len);
    b3dm.extend_from_slice(b"b3dm");
    b3dm.extend_from_slice(&1u32.to_le_bytes());
    b3dm.extend_from_slice(&(total_len as u32).to_le_bytes());
    b3dm.extend_from_slice(&(ft_json.len() as u32).to_le_bytes());
    b3dm.extend_from_slice(&0u32.to_le_bytes());
    b3dm.extend_from_slice(&0u32.to_le_bytes());
    b3dm.extend_from_slice(&0u32.to_le_bytes());
    b3dm.extend_from_slice(ft_json);
    b3dm.extend_from_slice(fake_glb);

    let payloads = extract_glb_payloads(&b3dm).expect("unpack b3dm");
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0].batch_length, 2);
    assert_eq!(payloads[0].rtc_center, DVec3::new(100.0, 200.0, 300.0));
    assert_eq!(&payloads[0].glb_bytes[0..4], b"glTF");
}

#[test]
fn tileset_lossless_round_trip_and_ray_pick() {
    let dir = std::env::temp_dir().join("3dtiles_editor_unit_test");
    let _ = std::fs::create_dir_all(&dir);
    let src_path = dir.join("tileset.json");
    let out_path = dir.join("tileset_saved.json");

    let json = r#"{
      "asset": { "version": "1.1", "customTag": "keep-me" },
      "geometricError": 50.0,
      "extensionsUsed": ["3DTILES_metadata"],
      "root": {
        "boundingVolume": { "box": [0, 0, 0, 10, 0, 0, 0, 10, 0, 0, 0, 10] },
        "geometricError": 10.0,
        "refine": "REPLACE",
        "extras": { "author": "gis-team" }
      }
    }"#;
    std::fs::write(&src_path, json).unwrap();

    let mut doc = TilesetDoc::open_file(&src_path).unwrap();
    doc.set_tile_trs(
        doc.root_id,
        Trs {
            translation: [50.0, 0.0, 0.0],
            rotation_deg: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        },
    )
    .unwrap();
    doc.set_tile_visible(doc.root_id, false).unwrap();

    // 验证 Ray Pick: 节点隐藏时不命中；恢复可见后命中偏移后的中心 [50, 0, 0]
    let candidates = vec![doc.root_id];
    assert!(doc
        .pick_ray(DVec3::new(50.0, 0.0, 100.0), DVec3::new(0.0, 0.0, -1.0), &candidates, &|_| None)
        .is_none());

    doc.set_tile_visible(doc.root_id, true).unwrap();
    let hit = doc
        .pick_ray(DVec3::new(50.0, 0.0, 100.0), DVec3::new(0.0, 0.0, -1.0), &candidates, &|_| None)
        .expect("should hit translated root box");
    assert_eq!(hit.tile_id, doc.root_id);

    doc.set_tile_visible(doc.root_id, false).unwrap();
    let saved_paths: Vec<PathBuf> = doc.save(Some(&out_path)).unwrap();
    assert_eq!(saved_paths.len(), 1);

    let saved_text = std::fs::read_to_string(&out_path).unwrap();
    assert!(saved_text.contains("\"customTag\": \"keep-me\""));
    assert!(saved_text.contains("\"author\": \"gis-team\""));
    assert!(saved_text.contains("\"3dtiles_editor\""));

    let _ = std::fs::remove_dir_all(&dir);
}
