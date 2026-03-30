use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use space_finder::{
    model::{EntryNode, FileCategory, ScanTree},
    scan::scan_paths,
};
use tempfile::tempdir;
#[test]
fn classifies_extensions() {
    assert_eq!(FileCategory::from_path("track.mp3".as_ref()), FileCategory::Audio);
    assert_eq!(FileCategory::from_path("movie.mkv".as_ref()), FileCategory::Video);
    assert_eq!(FileCategory::from_path("archive.zip".as_ref()), FileCategory::Archive);
    assert_eq!(FileCategory::from_path("report.pdf".as_ref()), FileCategory::Document);
    assert_eq!(FileCategory::from_path("main.rs".as_ref()), FileCategory::Code);
}
#[test]
fn scans_directory_and_sums_sizes() {
    let dir = tempdir().expect("temp dir");
    let nested = dir.path().join("nested");
    fs::create_dir(&nested).expect("nested dir");
    fs::write(dir.path().join("a.bin"), vec![0_u8; 10]).expect("file a");
    fs::write(nested.join("b.bin"), vec![0_u8; 20]).expect("file b");
    let scanned_counter = Arc::new(AtomicU64::new(0));
    let result = scan_paths(
        vec![dir.path().to_path_buf()],
        &Arc::new(AtomicBool::new(false)),
        &Arc::clone(&scanned_counter),
    );
    assert_eq!(result.total_size, 30);
    assert_eq!(result.file_count, 2);
    assert_eq!(scanned_counter.load(Ordering::Relaxed), 2);
    assert!(result.dir_count >= 2);
    assert!(!result.canceled);
    assert!(
        result
            .top_entries(8)
            .iter()
            .any(|entry| entry.path.ends_with("b.bin") && entry.size == 20)
    );
}
#[test]
fn honors_cancellation_before_scan() {
    let cancel = Arc::new(AtomicBool::new(true));
    let result = scan_paths(vec!["/tmp".into()], &cancel, &Arc::new(AtomicU64::new(0)));
    assert!(cancel.load(Ordering::Relaxed));
    assert!(result.canceled);
    assert!(result.roots.is_empty());
}

#[test]
fn top_entries_returns_files_only() {
    let mut tree = ScanTree {
        roots: vec![EntryNode::directory(
            "root".into(),
            vec![
                EntryNode::directory(
                    "root/folder".into(),
                    vec![EntryNode::file("root/folder/a.bin".into(), 200, 2)],
                    1,
                ),
                EntryNode::file("root/b.bin".into(), 100, 1),
            ],
            0,
        )],
        ..ScanTree::default()
    };
    tree.recompute();

    let top = tree.top_entries(8);

    assert_eq!(top.len(), 2);
    assert!(top.iter().all(|entry| entry.kind == "Plik"));
}

#[test]
fn top_entries_respects_zero_limit() {
    let mut tree = ScanTree {
        roots: vec![EntryNode::directory(
            "root".into(),
            vec![EntryNode::file("root/a.bin".into(), 42, 1)],
            0,
        )],
        ..ScanTree::default()
    };
    tree.recompute();

    assert!(tree.top_entries(0).is_empty());
}

#[test]
fn top_entries_are_stable_for_equal_sizes() {
    let mut tree = ScanTree {
        roots: vec![EntryNode::directory(
            "root".into(),
            vec![
                EntryNode::file("root/z.bin".into(), 100, 1),
                EntryNode::file("root/a.bin".into(), 100, 1),
                EntryNode::file("root/m.bin".into(), 100, 1),
            ],
            0,
        )],
        ..ScanTree::default()
    };
    tree.recompute();

    let top = tree.top_entries(3);
    let paths = top.into_iter().map(|entry| entry.path).collect::<Vec<_>>();
    assert_eq!(paths, vec!["root/a.bin", "root/m.bin", "root/z.bin"]);
}

#[test]
fn does_not_double_count_nested_roots() {
    let dir = tempdir().expect("temp dir");
    let nested = dir.path().join("nested");
    fs::create_dir(&nested).expect("nested dir");
    fs::write(dir.path().join("root.bin"), vec![0_u8; 10]).expect("root file");
    fs::write(nested.join("nested.bin"), vec![0_u8; 20]).expect("nested file");

    let result = scan_paths(
        vec![dir.path().to_path_buf(), nested],
        &Arc::new(AtomicBool::new(false)),
        &Arc::new(AtomicU64::new(0)),
    );

    assert_eq!(result.total_size, 30);
}

#[test]
fn does_not_double_count_same_root_twice() {
    let dir = tempdir().expect("temp dir");
    fs::write(dir.path().join("root.bin"), vec![0_u8; 64]).expect("root file");

    let result = scan_paths(
        vec![dir.path().to_path_buf(), dir.path().to_path_buf()],
        &Arc::new(AtomicBool::new(false)),
        &Arc::new(AtomicU64::new(0)),
    );

    assert_eq!(result.total_size, 64);
}
