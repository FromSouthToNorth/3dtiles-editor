use std::path::Path;
use std::time::{Duration, Instant};
use tiles_app::{AppCommand, EditorEvent, EngineSession};
use tiles_core::Trs;

#[test]
fn headless_session_opens_edits_and_saves_gygcdm() {
    let dataset = Path::new(r"D:\BaiduNetdiskDownload\SXNWDSMY\gygcdm\tileset.json");
    if !dataset.exists() {
        return;
    }

    let session = EngineSession::start(None).expect("start headless engine");
    session.send(AppCommand::OpenTileset(dataset.to_path_buf()));

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut opened_root = None;
    while Instant::now() < deadline {
        if let Some(evt) = session.poll_event() {
            if let EditorEvent::TilesetOpened { root_id, .. } = evt {
                opened_root = Some(root_id);
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let root_id = opened_root.expect("TilesetOpened event received");
    session.send(AppCommand::SetTileTrs {
        tile_id: root_id,
        trs: Trs {
            translation: [1.0, 2.0, 3.0],
            rotation_deg: [0.0, 0.0, 15.0],
            scale: [1.0, 1.0, 1.0],
        },
    });
    session.send(AppCommand::SetTileVisible {
        tile_id: root_id,
        visible: false,
    });
    std::thread::sleep(Duration::from_millis(50));

    let temp_out = std::env::temp_dir().join("3dtiles_editor_gygcdm_saved.json");
    session.send(AppCommand::SaveTileset(Some(temp_out.clone())));

    let save_deadline = Instant::now() + Duration::from_secs(3);
    let mut saved = false;
    while Instant::now() < save_deadline {
        if let Some(EditorEvent::TilesetSaved { files }) = session.poll_event() {
            assert_eq!(files.len(), 1);
            saved = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(saved, "expected TilesetSaved event");

    let saved_json = std::fs::read_to_string(&temp_out).unwrap();
    assert!(saved_json.contains("3dtiles_editor"));
    let _ = std::fs::remove_file(&temp_out);
}
