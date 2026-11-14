use std::path::Path;
use tiles_core::container::extract_glb_payloads;
use tiles_core::gltf_loader::load_glb_or_gltf;
use tiles_core::schema::Trs;
use tiles_core::TilesetDoc;

#[test]
fn gygcdm_tileset_parse_and_external_splice_smoke() {
    let dataset = Path::new(r"D:\BaiduNetdiskDownload\SXNWDSMY\gygcdm\tileset.json");
    if !dataset.exists() {
        return; // 非本机测试环境跳过
    }

    let mut doc = TilesetDoc::open_file(dataset).expect("open root tileset.json");
    println!("OPENED NODES COUNT = {}", doc.all_nodes().len()); let root_summary = doc.summary(doc.root_id).expect("root summary"); assert!(doc.all_nodes().len() > 10);
    assert_eq!(root_summary.refine, "REPLACE");
    assert!(root_summary.bv_radius > 3000.0);

    // 验证姿态本地 Up 向量（应与 ECEF 地表法线夹角 < 2度）
    let pose = doc.compute_focus_pose(None);
    let ecef_norm = pose.target.normalize();
    assert!(pose.local_up.dot(ecef_norm) > 0.999);

    // 验证 80B 占位 GLB 安全解包为空网格而非报错
    let d000 = Path::new(r"D:\BaiduNetdiskDownload\SXNWDSMY\gygcdm\Data\d000.glb");
    if d000.exists() {
        let bytes = std::fs::read(d000).unwrap();
        let glbs = extract_glb_payloads(&bytes).unwrap();
        let mesh = load_glb_or_gltf(glbs[0].glb_bytes, d000.parent(), glbs[0].rtc_center).unwrap();
        assert!(mesh.is_empty());
    }

    // 验证外部 tileset Data/e0351/tileset.json 与实景高模 e2301.glb
    let ext_node = doc
        .all_nodes()
        .iter()
        .find(|n| {
            n.external_tileset_path
                .as_ref()
                .map(|p| p.ends_with(r"e0351\tileset.json") || p.ends_with("e0351/tileset.json"))
                .unwrap_or(false)
        })
        .map(|n| (n.id, n.external_tileset_path.clone().unwrap()));

    if let Some((parent_id, ext_path)) = ext_node {
        let text = std::fs::read_to_string(&ext_path).unwrap();
        let ast = serde_json::from_str(&text).unwrap();
        let ext_root_id = doc.splice_external_tileset(parent_id, ext_path, ast).unwrap();
        assert!(!doc.children_summaries(ext_root_id).is_empty());
    }

    let e2301 = Path::new(r"D:\BaiduNetdiskDownload\SXNWDSMY\gygcdm\Data\e0351\e2301.glb");
    if e2301.exists() {
        let bytes = std::fs::read(e2301).unwrap();
        let glbs = extract_glb_payloads(&bytes).unwrap();
        let mesh = load_glb_or_gltf(glbs[0].glb_bytes, e2301.parent(), glbs[0].rtc_center).unwrap();
        assert!(!mesh.is_empty());
        assert!(!mesh.textures.is_empty());
    }

    // 验证编辑 TRS 与显隐不破坏 root 节点数据
    doc.set_tile_trs(
        doc.root_id,
        Trs {
            translation: [10.0, 20.0, 30.0],
            rotation_deg: [0.0, 0.0, 45.0],
            scale: [1.0, 1.0, 1.0],
        },
    )
    .unwrap();
    doc.set_tile_visible(doc.root_id, false).unwrap();
    let updated = doc.summary(doc.root_id).unwrap();
    assert!(!updated.visible);
}
