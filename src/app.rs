use std::{
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use rfd::FileDialog;
use slint::{Brush, ComponentHandle, ModelRc, SharedString, VecModel};

use crate::{
    MainWindow, ScanPathRow, TopEntryRow,
    config::AppConfig,
    localizer::setup_language,
    model::{FileCategory, ScanTree, TopEntry, detect_path_kind, format_bytes, merge_paths},
    render::{DEFAULT_RENDER_HEIGHT, DEFAULT_RENDER_WIDTH, HitMap, empty_chart, render_chart_with_hits},
    scan, t,
};
#[derive(Clone)]
struct ChartUiState {
    tree: Option<Arc<ScanTree>>,
    hit_map: HitMap,
    width: u32,
    height: u32,
    hovered_path: Option<String>,
    selected_path: Option<String>,
    context_path: Option<String>,
    view_path: Option<String>,
}
impl Default for ChartUiState {
    fn default() -> Self {
        Self {
            tree: None,
            hit_map: HitMap::default(),
            width: DEFAULT_RENDER_WIDTH,
            height: DEFAULT_RENDER_HEIGHT,
            hovered_path: None,
            selected_path: None,
            context_path: None,
            view_path: None,
        }
    }
}
pub fn run() -> Result<(), slint::PlatformError> {
    setup_language();
    let app = MainWindow::new()?;
    initialize_ui(&app);

    let config = AppConfig::load();
    let default_paths: Vec<PathBuf> = if !config.last_paths.is_empty() {
        log::info!("Loaded {} saved paths from config", config.last_paths.len());
        config.last_paths
    } else {
        std::env::var("HOME")
            .ok()
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .into_iter()
            .collect()
    };

    let selected_paths = Arc::new(Mutex::new(default_paths.clone()));
    if !default_paths.is_empty() {
        refresh_paths_model(&app, &default_paths);
        app.set_status_text(t!("status-default-path", path = default_paths[0].display().to_string()).into());
    }
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let scan_running = Arc::new(AtomicBool::new(false));
    let chart_state = Arc::new(Mutex::new(ChartUiState::default()));
    connect_path_management(&app, Arc::clone(&selected_paths));
    let weak_for_sources = app.as_weak();
    app.on_toggle_sources_requested(move || {
        if let Some(app) = weak_for_sources.upgrade() {
            app.set_sources_visible(!app.get_sources_visible());
        }
    });
    connect_scan_actions(
        &app,
        Arc::clone(&selected_paths),
        Arc::clone(&cancel_flag),
        Arc::clone(&scan_running),
        Arc::clone(&chart_state),
    );
    connect_chart_interactions(&app, &chart_state);
    app.run()
}
fn initialize_ui(app: &MainWindow) {
    app.set_scan_paths(ModelRc::new(VecModel::<ScanPathRow>::default()));
    app.set_top_entries(ModelRc::new(VecModel::<TopEntryRow>::default()));
    app.set_status_text(t!("status-add-paths").into());
    app.set_summary_text(t!("summary-no-results").into());
    app.set_chart_hover_text(t!("hover-hint").into());
    app.set_chart_hover_path("".into());
    app.set_has_results(false);
    app.set_context_menu_visible(false);
    app.set_chart_image(empty_chart(DEFAULT_RENDER_WIDTH, DEFAULT_RENDER_HEIGHT));
    app.set_folder_color(Brush::from(FileCategory::Folder.slint_color()));
    app.set_audio_color(Brush::from(FileCategory::Audio.slint_color()));
    app.set_video_color(Brush::from(FileCategory::Video.slint_color()));
    app.set_image_color(Brush::from(FileCategory::Image.slint_color()));
    app.set_archive_color(Brush::from(FileCategory::Archive.slint_color()));
    app.set_document_color(Brush::from(FileCategory::Document.slint_color()));
    app.set_code_color(Brush::from(FileCategory::Code.slint_color()));
    app.set_disk_image_color(Brush::from(FileCategory::DiskImage.slint_color()));
    app.set_binary_color(Brush::from(FileCategory::Binary.slint_color()));
    app.set_other_color(Brush::from(FileCategory::Other.slint_color()));
    set_ui_translations(app);
}

fn set_ui_translations(app: &MainWindow) {
    app.set_tr_sources(t!("ui-sources").into());
    app.set_tr_hide(t!("ui-hide").into());
    app.set_tr_show(t!("ui-show").into());
    app.set_tr_add_folders(t!("ui-add-folders").into());
    app.set_tr_add_files(t!("ui-add-files").into());
    app.set_tr_paste_paths(t!("ui-paste-paths").into());
    app.set_tr_add(t!("ui-add").into());
    app.set_tr_clear(t!("ui-clear").into());
    app.set_tr_selected(t!("ui-selected").into());
    app.set_tr_empty(t!("ui-empty").into());
    app.set_tr_start(t!("ui-start").into());
    app.set_tr_stop(t!("ui-stop").into());
    app.set_tr_visualization(t!("ui-visualization").into());
    app.set_tr_treemap_desc(t!("ui-treemap-desc").into());
    app.set_tr_top_files(t!("ui-top-files").into());
    app.set_tr_after_scan(t!("ui-after-scan").into());
    app.set_tr_open(t!("ui-open").into());
    app.set_tr_open_parent(t!("ui-open-parent").into());
    app.set_tr_scanning(t!("ui-scanning").into());
    app.set_tr_color_legend(t!("ui-color-legend").into());
    app.set_tr_mode_scanning(t!("ui-mode-scanning").into());
    app.set_tr_mode_results(t!("ui-mode-results").into());
    app.set_tr_mode_waiting(t!("ui-mode-waiting").into());
}
fn connect_path_management(app: &MainWindow, selected_paths: Arc<Mutex<Vec<PathBuf>>>) {
    let weak = app.as_weak();
    let selected_paths_for_folders = Arc::clone(&selected_paths);
    app.on_add_folders_requested(move || {
        let weak = weak.clone();
        let selected_paths = Arc::clone(&selected_paths_for_folders);
        std::thread::spawn(move || {
            let start_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
            if let Some(folders) = FileDialog::new().set_directory(start_dir).pick_folders() {
                let snapshot = {
                    let mut guard = selected_paths.lock().expect("selected paths poisoned");
                    merge_paths(&mut guard, folders);
                    guard.clone()
                };
                let _ = weak.upgrade_in_event_loop(move |app| {
                    refresh_paths_model(&app, &snapshot);
                    app.set_status_text(t!("status-paths-selected", count = snapshot.len()).into());
                    AppConfig {
                        last_paths: snapshot.clone(),
                    }
                    .save();
                    log::info!("Paths updated: {} total", snapshot.len());
                });
            }
        });
    });
    let weak = app.as_weak();
    let selected_paths_for_files = Arc::clone(&selected_paths);
    app.on_add_files_requested(move || {
        let weak = weak.clone();
        let selected_paths = Arc::clone(&selected_paths_for_files);
        std::thread::spawn(move || {
            let start_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
            if let Some(files) = FileDialog::new().set_directory(start_dir).pick_files() {
                let snapshot = {
                    let mut guard = selected_paths.lock().expect("selected paths poisoned");
                    merge_paths(&mut guard, files);
                    guard.clone()
                };
                let _ = weak.upgrade_in_event_loop(move |app| {
                    refresh_paths_model(&app, &snapshot);
                    app.set_status_text(t!("status-paths-selected", count = snapshot.len()).into());
                    AppConfig {
                        last_paths: snapshot.clone(),
                    }
                    .save();
                    log::info!("Paths updated: {} total", snapshot.len());
                });
            }
        });
    });
    let weak = app.as_weak();
    let selected_paths_for_manual = Arc::clone(&selected_paths);
    app.on_apply_manual_paths(move |manual_input| {
        let manual_paths = manual_input
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        if manual_paths.is_empty() {
            return;
        }
        let snapshot = {
            let mut guard = selected_paths_for_manual.lock().expect("selected paths poisoned");
            merge_paths(&mut guard, manual_paths);
            guard.clone()
        };
        if let Some(app) = weak.upgrade() {
            refresh_paths_model(&app, &snapshot);
            app.set_manual_paths(SharedString::new());
            app.set_status_text(t!("status-paths-manual", count = snapshot.len()).into());
            AppConfig { last_paths: snapshot }.save();
        }
    });
    let weak = app.as_weak();
    let selected_paths_for_remove = Arc::clone(&selected_paths);
    app.on_remove_path_requested(move |index| {
        let snapshot = {
            let mut guard = selected_paths_for_remove.lock().expect("selected paths poisoned");
            if index >= 0 && (index as usize) < guard.len() {
                guard.remove(index as usize);
            }
            guard.clone()
        };
        if let Some(app) = weak.upgrade() {
            refresh_paths_model(&app, &snapshot);
            let status = if snapshot.is_empty() {
                t!("status-paths-empty")
            } else {
                t!("status-paths-remaining", count = snapshot.len())
            };
            app.set_status_text(status.into());
            AppConfig { last_paths: snapshot }.save();
        }
    });
    let weak = app.as_weak();
    app.on_clear_paths_requested(move || {
        if let Ok(mut guard) = selected_paths.lock() {
            guard.clear();
        }
        if let Some(app) = weak.upgrade() {
            refresh_paths_model(&app, &[]);
            app.set_status_text(t!("status-paths-cleared").into());
            AppConfig { last_paths: vec![] }.save();
            log::info!("Path list cleared");
        }
    });
}
fn connect_scan_actions(
    app: &MainWindow,
    selected_paths: Arc<Mutex<Vec<PathBuf>>>,
    cancel_flag: Arc<AtomicBool>,
    scan_running: Arc<AtomicBool>,
    chart_state: Arc<Mutex<ChartUiState>>,
) {
    let stop_flag = Arc::clone(&cancel_flag);
    app.on_stop_scan_requested(move || {
        stop_flag.store(true, Ordering::Relaxed);
    });
    let weak = app.as_weak();
    app.on_start_scan_requested(move || {
        if scan_running.swap(true, Ordering::SeqCst) {
            return;
        }
        let paths = selected_paths.lock().expect("selected paths poisoned").clone();
        if paths.is_empty() {
            scan_running.store(false, Ordering::SeqCst);
            if let Some(app) = weak.upgrade() {
                app.set_status_text(t!("status-add-one-path").into());
            }
            return;
        }
        cancel_flag.store(false, Ordering::Relaxed);
        if let Ok(mut state) = chart_state.lock() {
            state.tree = None;
            state.hit_map = HitMap::default();
            state.hovered_path = None;
            state.selected_path = None;
            state.context_path = None;
            state.view_path = None;
        }
        if let Some(app) = weak.upgrade() {
            app.set_scanning(true);
            app.set_sources_visible(false);
            app.set_has_results(false);
            app.set_summary_text(t!("status-scanning-progress").into());
            app.set_chart_hover_text(t!("hover-hint").into());
            app.set_chart_hover_path("".into());
            app.set_context_menu_visible(false);
            app.set_top_entries(ModelRc::new(VecModel::<TopEntryRow>::default()));
            app.set_chart_image(empty_chart(DEFAULT_RENDER_WIDTH, DEFAULT_RENDER_HEIGHT));
            app.set_status_text(t!("status-scanning", count = 0u64).into());
        }
        let scanned_counter = Arc::new(AtomicU64::new(0));
        let weak_for_progress = weak.clone();
        let scan_running_for_progress = Arc::clone(&scan_running);
        let scanned_counter_for_progress = Arc::clone(&scanned_counter);
        std::thread::spawn(move || {
            while scan_running_for_progress.load(Ordering::Relaxed) {
                let scanned = scanned_counter_for_progress.load(Ordering::Relaxed);
                let _ = weak_for_progress.upgrade_in_event_loop(move |app| {
                    if app.get_scanning() {
                        app.set_status_text(t!("status-scanning", count = scanned).into());
                    }
                });
                std::thread::sleep(Duration::from_millis(160));
            }
        });
        let weak_for_finish = weak.clone();
        let cancel_flag = Arc::clone(&cancel_flag);
        let scan_running = Arc::clone(&scan_running);
        let chart_state_for_finish = Arc::clone(&chart_state);
        let scanned_counter_for_scan = Arc::clone(&scanned_counter);
        std::thread::spawn(move || {
            let result = scan::scan_paths(paths, &cancel_flag, &scanned_counter_for_scan);
            scan_running.store(false, Ordering::SeqCst);
            let final_count = scanned_counter.load(Ordering::Relaxed);
            let _ = weak_for_finish.upgrade_in_event_loop(move |app| {
                let result = Arc::new(result);
                if let Ok(mut state) = chart_state_for_finish.lock() {
                    state.tree = Some(result.clone());
                    state.hovered_path = None;
                    state.context_path = None;
                }
                refresh_result_views(&app, &Arc::clone(&chart_state_for_finish));
                app.set_scanning(false);
                app.set_status_text(
                    if result.canceled {
                        t!("status-scan-canceled", count = final_count)
                    } else {
                        t!("status-scan-done", count = final_count)
                    }
                    .into(),
                );
            });
        });
    });
}
fn connect_chart_interactions(app: &MainWindow, chart_state: &Arc<Mutex<ChartUiState>>) {
    let weak = app.as_weak();
    let chart_state_resize = Arc::clone(chart_state);
    app.on_chart_area_resized(move |width, height| {
        let width = width.max(1.0) as u32;
        let height = height.max(1.0) as u32;
        let should_refresh = if let Ok(mut state) = chart_state_resize.lock()
            && (state.width != width || state.height != height)
        {
            state.width = width;
            state.height = height;
            true
        } else {
            false
        };
        if should_refresh && let Some(app) = weak.upgrade() {
            refresh_chart_view(&app, &Arc::clone(&chart_state_resize));
        }
    });
    let weak = app.as_weak();
    let chart_state_hover = Arc::clone(chart_state);
    app.on_chart_hovered(move |x, y| {
        if let Some(app) = weak.upgrade() {
            let mut should_refresh = false;
            if let Ok(mut state) = chart_state_hover.lock() {
                let new_hover = if let Some(hit) = state.hit_map.hit_test(x, y) {
                    app.set_chart_hover_text(hit.line1().into());
                    app.set_chart_hover_path(hit.path.clone().into());
                    Some(hit.path)
                } else {
                    app.set_chart_hover_text(t!("hover-hint").into());
                    app.set_chart_hover_path("".into());
                    None
                };
                if state.hovered_path != new_hover {
                    state.hovered_path = new_hover;
                    should_refresh = true;
                }
            }
            if should_refresh {
                refresh_chart_view(&app, &Arc::clone(&chart_state_hover));
            }
        }
    });
    let weak = app.as_weak();
    let chart_state_left = Arc::clone(chart_state);
    app.on_chart_left_clicked(move |x, y| {
        if let Some(app) = weak.upgrade() {
            let mut should_refresh = false;
            if let Ok(mut state) = chart_state_left.lock() {
                state.context_path = None;
                let hit = state.hit_map.hit_test(x, y);
                let new_path = hit.as_ref().map(|h| h.path.clone());
                if let Some(ref h) = hit {
                    app.set_chart_hover_text(h.line1().into());
                    app.set_chart_hover_path(h.path.clone().into());
                } else {
                    app.set_chart_hover_text(t!("hover-hint").into());
                    app.set_chart_hover_path("".into());
                }
                // Update both hover and selection so old hover highlight disappears.
                let hover_changed = state.hovered_path != new_path;
                let select_changed = state.selected_path != new_path;
                state.hovered_path = new_path.clone();
                state.selected_path = new_path;
                if hover_changed || select_changed {
                    should_refresh = true;
                }
            }
            app.set_context_menu_visible(false);
            if should_refresh {
                refresh_chart_view(&app, &Arc::clone(&chart_state_left));
            }
        }
    });
    let weak = app.as_weak();
    let chart_state_right = Arc::clone(chart_state);
    app.on_chart_right_clicked(move |x, y| {
        if let Some(app) = weak.upgrade() {
            let mut should_refresh = false;
            if let Ok(mut state) = chart_state_right.lock() {
                if let Some(hit) = state.hit_map.hit_test(x, y) {
                    let selected = Some(hit.path.clone());
                    let hover_changed = state.hovered_path != selected;
                    let select_changed = state.selected_path != selected;
                    state.hovered_path = selected.clone();
                    state.selected_path = selected;
                    if hover_changed || select_changed {
                        should_refresh = true;
                    }
                    state.context_path = Some(hit.path.clone());
                    app.set_context_menu_title(format!("{} | {}", hit.label, format_bytes(hit.size)).into());
                    app.set_context_menu_x(x);
                    app.set_context_menu_y(y);
                    app.set_context_menu_visible(true);
                    app.set_chart_hover_text(hit.line1().into());
                    app.set_chart_hover_path(hit.path.into());
                } else {
                    // Clear hover and context when right-clicking empty space.
                    let prev_hover = state.hovered_path.take();
                    if prev_hover.is_some() {
                        should_refresh = true;
                    }
                    state.context_path = None;
                    app.set_context_menu_visible(false);
                    app.set_chart_hover_text(t!("hover-hint").into());
                    app.set_chart_hover_path("".into());
                }
            }
            if should_refresh {
                refresh_chart_view(&app, &Arc::clone(&chart_state_right));
            }
        }
    });
    let weak = app.as_weak();
    app.on_close_context_menu_requested(move || {
        if let Some(app) = weak.upgrade() {
            app.set_context_menu_visible(false);
        }
    });
    // Scroll up = drill down toward selected path.
    let weak = app.as_weak();
    let chart_state_scroll_up = Arc::clone(chart_state);
    app.on_chart_scroll_up(move || {
        let new_view = {
            if let Ok(state) = chart_state_scroll_up.lock() {
                if let Some(ref selected) = state.selected_path {
                    next_view_path_toward(state.view_path.as_deref(), selected, state.tree.as_deref())
                } else {
                    return;
                }
            } else {
                return;
            }
        };
        if let Ok(mut state) = chart_state_scroll_up.lock() {
            if state.view_path == new_view {
                return;
            }
            state.view_path = new_view;
        }
        if let Some(app) = weak.upgrade() {
            refresh_chart_view(&app, &Arc::clone(&chart_state_scroll_up));
            if let Ok(state) = chart_state_scroll_up.lock()
                && let Some(ref vp) = state.view_path
            {
                app.set_status_text(t!("status-view", path = vp.clone()).into());
            }
        }
    });
    // Scroll down = zoom out (go to parent folder).
    let weak = app.as_weak();
    let chart_state_scroll_down = Arc::clone(chart_state);
    app.on_chart_scroll_down(move || {
        let new_view = {
            if let Ok(state) = chart_state_scroll_down.lock() {
                if let Some(ref vp) = state.view_path {
                    prev_view_path(vp)
                } else {
                    return; // already at root
                }
            } else {
                return;
            }
        };
        if let Ok(mut state) = chart_state_scroll_down.lock() {
            state.view_path = new_view;
        }
        if let Some(app) = weak.upgrade() {
            refresh_chart_view(&app, &Arc::clone(&chart_state_scroll_down));
            if let Ok(state) = chart_state_scroll_down.lock() {
                let status = match &state.view_path {
                    Some(p) => t!("status-view", path = p.clone()),
                    None => t!("status-view-root"),
                };
                app.set_status_text(status.into());
            }
        }
    });
    let weak = app.as_weak();
    let chart_state_open = Arc::clone(chart_state);
    app.on_open_item_requested(move |open_parent| {
        if let Some(app) = weak.upgrade() {
            let target = chart_state_open
                .lock()
                .ok()
                .and_then(|state| state.context_path.clone());
            if let Some(path) = target {
                match open_path_target(&path, open_parent) {
                    Ok(()) => {
                        app.set_status_text(
                            if open_parent {
                                t!("status-opening-parent", path = path.clone())
                            } else {
                                t!("status-opening", path = path.clone())
                            }
                            .into(),
                        );
                    }
                    Err(error) => {
                        app.set_status_text(t!("status-open-failed", error = error).into());
                    }
                }
            }
            app.set_context_menu_visible(false);
        }
    });
    let weak = app.as_weak();
    let chart_state_dbl = Arc::clone(chart_state);
    app.on_chart_double_clicked(move |x, y| {
        let hit = chart_state_dbl.lock().ok().and_then(|s| s.hit_map.hit_test(x, y));
        if let Some(hit) = hit {
            log::info!("Double-click open: {}", hit.path);
            if let Some(app) = weak.upgrade() {
                match open_path_target(&hit.path, false) {
                    Ok(()) => app.set_status_text(t!("status-opening", path = hit.path.clone()).into()),
                    Err(e) => app.set_status_text(t!("status-open-failed", error = e).into()),
                }
            }
        }
    });
}
fn refresh_result_views(app: &MainWindow, chart_state: &Arc<Mutex<ChartUiState>>) {
    let tree = {
        let state = chart_state.lock().expect("chart state poisoned");
        let Some(tree) = state.tree.as_ref() else {
            app.set_has_results(false);
            return;
        };
        Arc::clone(tree)
    };
    let top_rows = tree
        .top_entries(32)
        .into_iter()
        .map(top_entry_to_row)
        .collect::<Vec<_>>();
    app.set_has_results(!tree.is_empty());
    app.set_summary_text(summary_text(&tree).into());
    app.set_top_entries(Rc::new(VecModel::from(top_rows)).into());
    refresh_chart_view(app, chart_state);
}

fn refresh_chart_view(app: &MainWindow, chart_state: &Arc<Mutex<ChartUiState>>) {
    let (tree, width, height, hovered_path, selected_path, view_path) = {
        let state = chart_state.lock().expect("chart state poisoned");
        let Some(tree) = state.tree.as_ref() else {
            app.set_has_results(false);
            return;
        };
        (
            Arc::clone(tree),
            state.width.max(1),
            state.height.max(1),
            state.hovered_path.clone(),
            state.selected_path.clone(),
            state.view_path.clone(),
        )
    };
    app.set_has_results(!tree.is_empty());
    let rendered = render_chart_with_hits(
        &tree,
        width,
        height,
        hovered_path.as_deref(),
        selected_path.as_deref(),
        view_path.as_deref(),
    );
    if let Ok(mut state) = chart_state.lock() {
        state.hit_map = rendered.hit_map;
    }
    app.set_chart_image(rendered.image);
}
fn open_path_target(path: &str, open_parent: bool) -> Result<(), String> {
    let base = Path::new(path);
    let target = if open_parent {
        base.parent().unwrap_or(base)
    } else {
        base
    };
    open::that(target).map_err(|error| error.to_string())
}
fn refresh_paths_model(app: &MainWindow, paths: &[PathBuf]) {
    let rows = paths
        .iter()
        .map(|path| ScanPathRow {
            path: path.to_string_lossy().to_string().into(),
            kind_label: detect_path_kind(path).into(),
        })
        .collect::<Vec<_>>();
    app.set_scan_paths(Rc::new(VecModel::from(rows)).into());
}
fn top_entry_to_row(entry: TopEntry) -> TopEntryRow {
    let name = std::path::Path::new(&entry.path)
        .file_name()
        .map_or_else(|| entry.path.clone(), |n| n.to_string_lossy().to_string());
    TopEntryRow {
        name: name.into(),
        path: entry.path.into(),
        size_label: format_bytes(entry.size).into(),
        meta: entry.category.label().into(),
    }
}
fn summary_text(tree: &ScanTree) -> String {
    let size = format_bytes(tree.total_size);
    if tree.canceled {
        t!(
            "summary-total-partial",
            size = size,
            files = tree.file_count,
            folders = tree.dir_count,
            warnings = tree.warnings,
            entries = tree.scanned_entries
        )
    } else {
        t!(
            "summary-total",
            size = size,
            files = tree.file_count,
            folders = tree.dir_count,
            warnings = tree.warnings,
            entries = tree.scanned_entries
        )
    }
}

/// Return the next view_path one level deeper toward `selected`, given the current `view_path`.
fn next_view_path_toward(view_path: Option<&str>, selected: &str, tree: Option<&ScanTree>) -> Option<String> {
    let selected_path = Path::new(selected);
    // Build ancestor chain from selected upward: [selected, parent, ..., root]
    let ancestors: Vec<&Path> = selected_path.ancestors().collect();

    if let Some(vp) = view_path {
        let vp_path = Path::new(vp);
        // Find vp in the chain; one step deeper is at index i-1.
        for (i, ancestor) in ancestors.iter().enumerate() {
            if *ancestor == vp_path {
                if i > 0 {
                    let next = ancestors[i - 1];
                    // Only navigate into directories — files cannot be a view root.
                    if next.is_dir() {
                        return Some(next.to_string_lossy().to_string());
                    }
                }
                // Can't go deeper (already at selected, or next step is a file).
                // Return the current view unchanged so the caller treats this as a no-op.
                return Some(vp.to_string());
            }
        }
        None
    } else {
        // At root. Find the root node that contains selected, then go one level deeper into it.
        if let Some(tree) = tree {
            for root in &tree.roots {
                if selected_path.starts_with(&root.path) {
                    // If selected IS this root, can't go deeper.
                    if selected_path == root.path.as_path() {
                        return None;
                    }
                    // Find the direct child of root that is an ancestor of selected.
                    for ancestor in &ancestors {
                        if let Some(parent) = ancestor.parent()
                            && parent == root.path.as_path()
                        {
                            // Only descend into directories.
                            if ancestor.is_dir() {
                                return Some(ancestor.to_string_lossy().to_string());
                            }
                            return None; // file directly in root — stay at virtual root
                        }
                    }
                    // selected is directly inside root
                    return None;
                }
            }
        }
        // Fallback: second-to-last ancestor = first child of root
        if ancestors.len() >= 2 {
            let candidate = ancestors[ancestors.len() - 2];
            if candidate.is_dir() {
                Some(candidate.to_string_lossy().to_string())
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Return the parent path when zooming out, or None if already at root level.
fn prev_view_path(view_path: &str) -> Option<String> {
    let path = Path::new(view_path);
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_string_lossy().to_string())
}
