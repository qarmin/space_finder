use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use rayon::prelude::*;

use crate::model::{EntryNode, ScanTree};
#[derive(Debug, Default, Clone, Copy)]
struct LocalStats {
    file_count: u64,
    dir_count: u64,
    warnings: u64,
    scanned_entries: u64,
}
impl LocalStats {
    fn merge(&mut self, other: Self) {
        self.file_count = self.file_count.saturating_add(other.file_count);
        self.dir_count = self.dir_count.saturating_add(other.dir_count);
        self.warnings = self.warnings.saturating_add(other.warnings);
        self.scanned_entries = self.scanned_entries.saturating_add(other.scanned_entries);
    }
}
fn prepare_scan_roots(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut canonicalized = Vec::with_capacity(paths.len());
    for path in paths {
        let canonical = fs::canonicalize(&path).unwrap_or(path);
        if !canonicalized.iter().any(|existing| existing == &canonical) {
            canonicalized.push(canonical);
        }
    }
    canonicalized.sort_by(|left, right| {
        left.components()
            .count()
            .cmp(&right.components().count())
            .then_with(|| left.cmp(right))
    });
    let mut filtered: Vec<PathBuf> = Vec::with_capacity(canonicalized.len());
    for path in canonicalized {
        if filtered
            .iter()
            .any(|root: &PathBuf| is_same_or_nested(&path, root.as_path()))
        {
            continue;
        }
        filtered.push(path);
    }
    filtered
}
fn is_same_or_nested(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}
fn scan_path_parallel(
    path: PathBuf,
    depth: usize,
    cancel: &Arc<AtomicBool>,
    scanned_counter: &Arc<AtomicU64>,
) -> (Option<EntryNode>, LocalStats) {
    if cancel.load(Ordering::Relaxed) {
        return (None, LocalStats::default());
    }
    let mut stats = LocalStats::default();
    let Ok(metadata) = fs::symlink_metadata(&path) else {
        stats.warnings = stats.warnings.saturating_add(1);
        stats.scanned_entries = stats.scanned_entries.saturating_add(1);
        return (Some(EntryNode::inaccessible(path, depth)), stats);
    };
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        stats.scanned_entries = stats.scanned_entries.saturating_add(1);
        return (Some(EntryNode::symlink(path, depth)), stats);
    }
    if file_type.is_file() {
        stats.file_count = stats.file_count.saturating_add(1);
        stats.scanned_entries = stats.scanned_entries.saturating_add(1);
        scanned_counter.fetch_add(1, Ordering::Relaxed);
        return (Some(EntryNode::file(path, metadata.len(), depth)), stats);
    }
    if file_type.is_dir() {
        stats.dir_count = stats.dir_count.saturating_add(1);
        stats.scanned_entries = stats.scanned_entries.saturating_add(1);
        let Ok(read_dir) = fs::read_dir(&path) else {
            stats.warnings = stats.warnings.saturating_add(1);
            return (Some(EntryNode::inaccessible(path, depth)), stats);
        };
        let mut child_paths = Vec::new();
        for child in read_dir {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            match child {
                Ok(entry) => child_paths.push(entry.path()),
                Err(_) => {
                    stats.warnings = stats.warnings.saturating_add(1);
                }
            }
        }
        let child_results = child_paths
            .into_par_iter()
            .map(|child_path| scan_path_parallel(child_path, depth + 1, cancel, scanned_counter))
            .collect::<Vec<_>>();
        let mut children = Vec::new();
        for (child_node, child_stats) in child_results {
            stats.merge(child_stats);
            if let Some(node) = child_node {
                children.push(node);
            }
        }
        return (Some(EntryNode::directory(path, children, depth)), stats);
    }
    stats.scanned_entries = stats.scanned_entries.saturating_add(1);
    scanned_counter.fetch_add(1, Ordering::Relaxed);
    (Some(EntryNode::file(path, metadata.len(), depth)), stats)
}
pub fn scan_paths(paths: Vec<PathBuf>, cancel: &Arc<AtomicBool>, scanned_counter: &Arc<AtomicU64>) -> ScanTree {
    let roots_to_scan = prepare_scan_roots(paths);
    scanned_counter.store(0, Ordering::Relaxed);
    let root_results = roots_to_scan
        .into_par_iter()
        .map(|root| scan_path_parallel(root, 0, cancel, scanned_counter))
        .collect::<Vec<_>>();
    let mut tree = ScanTree::default();
    for (root, stats) in root_results {
        if let Some(node) = root {
            tree.roots.push(node);
        }
        tree.file_count = tree.file_count.saturating_add(stats.file_count);
        tree.dir_count = tree.dir_count.saturating_add(stats.dir_count);
        tree.warnings = tree.warnings.saturating_add(stats.warnings);
        tree.scanned_entries = tree.scanned_entries.saturating_add(stats.scanned_entries);
    }
    tree.canceled = cancel.load(Ordering::Relaxed);
    tree.recompute();
    tree
}
