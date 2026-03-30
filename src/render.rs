use std::path::Path;

use image::{Rgba, RgbaImage};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

use crate::model::{EntryNode, FileCategory, NodeKind, ScanTree, display_name, format_bytes, kind_label};
pub const DEFAULT_RENDER_WIDTH: u32 = 1800;
pub const DEFAULT_RENDER_HEIGHT: u32 = 1100;
const CHILD_GROUP_RATIO: f64 = 0.000001;
const MIN_OTHER_RATIO: f64 = 0.00001;
const HATCH_SPACING: i32 = 9;
const HATCH_THICKNESS: i32 = 2;
#[derive(Debug, Clone)]
pub struct ChartHit {
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    pub category: FileCategory,
    pub label: String,
}
impl ChartHit {
    pub fn line1(&self) -> String {
        if self.is_dir {
            format!("{} | {}", self.label, format_bytes(self.size))
        } else {
            format!(
                "{} | {} | {}",
                self.label,
                self.category.label(),
                format_bytes(self.size)
            )
        }
    }
    pub fn summary(&self) -> String {
        if self.is_dir {
            format!("{} | {} | {}", self.label, format_bytes(self.size), self.path)
        } else {
            format!(
                "{} | {} | {} | {}",
                self.label,
                self.category.label(),
                format_bytes(self.size),
                self.path,
            )
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct HitMap {
    regions: Vec<HitRegion>,
}
impl HitMap {
    pub fn hit_test(&self, x: f32, y: f32) -> Option<ChartHit> {
        for region in self.regions.iter().rev() {
            if let Some(hit) = region.hit_test(x, y) {
                return Some(hit);
            }
        }
        None
    }
}
#[derive(Debug, Clone)]
enum HitRegion {
    Rect { rect: Rect, hit: ChartHit },
}
impl HitRegion {
    fn hit_test(&self, x: f32, y: f32) -> Option<ChartHit> {
        match self {
            Self::Rect { rect, hit } => rect.contains(x, y).then(|| hit.clone()),
        }
    }
}
#[derive(Debug, Clone)]
pub struct RenderedChart {
    pub image: Image,
    pub hit_map: HitMap,
}
#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[derive(Debug, Clone, Copy)]
enum TreemapItem<'a> {
    Node(&'a EntryNode),
    Other { parent_path: &'a str, size: u64 },
}
impl Rect {
    fn inset(self, amount: f32) -> Self {
        let amount = amount.max(0.0);
        Self {
            x: self.x + amount,
            y: self.y + amount,
            w: (self.w - amount * 2.0).max(0.0),
            h: (self.h - amount * 2.0).max(0.0),
        }
    }
    fn area(self) -> f32 {
        self.w.max(0.0) * self.h.max(0.0)
    }
    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
}
pub fn empty_chart(width: u32, height: u32) -> Image {
    let mut image = RgbaImage::from_pixel(width.max(1), height.max(1), rgba([16, 20, 28, 255]));
    let stripe = rgba([22, 28, 38, 255]);
    for y in (0..height).step_by(22) {
        for x in 0..width {
            image.put_pixel(x, y, stripe);
        }
    }
    to_slint_image(&image)
}
pub fn render_chart(tree: &ScanTree, width: u32, height: u32) -> Image {
    render_chart_with_hits(tree, width, height, None, None, None).image
}
pub fn render_chart_with_hits(
    tree: &ScanTree,
    width: u32,
    height: u32,
    hovered_path: Option<&str>,
    selected_path: Option<&str>,
    view_path: Option<&str>,
) -> RenderedChart {
    if tree.is_empty() {
        return RenderedChart {
            image: empty_chart(width, height),
            hit_map: HitMap::default(),
        };
    }
    render_treemap(tree, width, height, hovered_path, selected_path, view_path)
}
fn render_treemap(
    tree: &ScanTree,
    width: u32,
    height: u32,
    hovered_path: Option<&str>,
    selected_path: Option<&str>,
    view_path: Option<&str>,
) -> RenderedChart {
    let mut image = RgbaImage::from_pixel(width.max(1), height.max(1), rgba([14, 18, 26, 255]));
    let root_rect = Rect {
        x: 3.0,
        y: 3.0,
        w: width.saturating_sub(6) as f32,
        h: height.saturating_sub(6) as f32,
    };
    let mut hit_map = HitMap::default();
    fill_rect(&mut image, root_rect, rgba([18, 23, 32, 255]));
    let top_level = view_root_items(tree, view_path);
    for (item, child_rect) in squarify(top_level, root_rect) {
        draw_treemap_item(
            item,
            child_rect,
            1,
            &mut image,
            &mut hit_map,
            hovered_path,
            selected_path,
        );
    }
    RenderedChart {
        image: to_slint_image(&image),
        hit_map,
    }
}
/// Draw diagonal (top-left → bottom-right) hatch lines over a rect.
fn draw_hatch(image: &mut RgbaImage, rect: Rect, hatch_color: [u8; 4]) {
    let x0 = rect.x.max(0.0).floor() as i32;
    let y0 = rect.y.max(0.0).floor() as i32;
    let x1 = (rect.x + rect.w).min(image.width() as f32 - 1.0).ceil() as i32;
    let y1 = (rect.y + rect.h).min(image.height() as f32 - 1.0).ceil() as i32;
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let alpha = hatch_color[3] as f32 / 255.0;
    for py in y0..=y1 {
        for px in x0..=x1 {
            let diag = (px - x0 + py - y0).rem_euclid(HATCH_SPACING);
            if diag < HATCH_THICKNESS {
                let existing = image.get_pixel(px as u32, py as u32).0;
                let blended = [
                    (existing[0] as f32 * (1.0 - alpha) + hatch_color[0] as f32 * alpha) as u8,
                    (existing[1] as f32 * (1.0 - alpha) + hatch_color[1] as f32 * alpha) as u8,
                    (existing[2] as f32 * (1.0 - alpha) + hatch_color[2] as f32 * alpha) as u8,
                    255,
                ];
                image.put_pixel(px as u32, py as u32, rgba(blended));
            }
        }
    }
}

/// Draw a folder as a leaf rectangle:
///  - horizontal gradient left-to-right showing category composition of contents
///  - diagonal black hatch lines on top to clearly distinguish it from a file
fn draw_folder_leaf(
    node: &EntryNode,
    rect: Rect,
    depth: usize,
    image: &mut RgbaImage,
    hit_map: &mut HitMap,
    hovered_path: Option<&str>,
    selected_path: Option<&str>,
) {
    if rect.w < 1.0 || rect.h < 1.0 || node.size == 0 {
        return;
    }
    let node_path = &node.path_str;
    let is_selected = selected_path.is_some_and(|p| p == node_path);
    let is_hovered = hovered_path.is_some_and(|p| p == node_path);

    let total_weight: u64 = node.category_weights.iter().sum();

    if total_weight == 0 || rect.w < 6.0 {
        // Too small or empty — plain folder colour
        let mut fill = shade(FileCategory::Folder.color(), 0.60 + depth as f32 * 0.04);
        fill = apply_highlight(fill, is_hovered, is_selected);
        fill_rect(image, rect, rgba(fill));
    } else {
        // Collect non-zero categories (FileCategory::Folder always has weight 0 here)
        let mut cats: Vec<(FileCategory, u64)> = FileCategory::ALL
            .iter()
            .filter_map(|cat| {
                let w = node.category_weights[cat.index()];
                if w > 0 { Some((*cat, w)) } else { None }
            })
            .collect();
        // Largest category leftmost
        cats.sort_unstable_by_key(|b| std::cmp::Reverse(b.1));

        let mut cursor_x = rect.x;
        for (i, (cat, weight)) in cats.iter().enumerate() {
            let band_w = if i == cats.len() - 1 {
                // Last band: fill remaining pixels exactly (avoids float rounding gaps)
                (rect.x + rect.w) - cursor_x
            } else {
                rect.w * (*weight as f32 / total_weight as f32)
            };
            if band_w <= 0.0 {
                continue;
            }
            let band = Rect {
                x: cursor_x,
                y: rect.y,
                w: band_w.max(0.0),
                h: rect.h,
            };
            let mut fill = shade(cat.color(), 0.78 + depth as f32 * 0.04);
            fill = apply_highlight(fill, is_hovered, is_selected);
            fill_rect(image, band, rgba(fill));
            cursor_x += band_w;
        }
    }

    // Diagonal black hatch — marks this rect as a folder
    draw_hatch(image, rect, [0, 0, 0, 75]);

    // Border
    let border = if is_selected {
        [255, 255, 255, 255]
    } else if is_hovered {
        [255, 255, 255, 170]
    } else {
        [0, 0, 0, 55]
    };
    stroke_rect(image, rect, rgba(border));
    if is_selected {
        stroke_rect(image, rect.inset(1.0), rgba([255, 255, 255, 210]));
    }

    hit_map.regions.push(HitRegion::Rect {
        rect,
        hit: make_hit(node_path, node.size, true, node.dominant_category),
    });
}

/// Recursively draw a directory node.
/// - If the folder has individually-renderable children → draw them recursively (show files).
/// - If ALL children are below the grouping threshold (too small) → draw the whole folder as
///   a gradient+hatch leaf so the user still sees *something* meaningful.
fn draw_grouped_node(
    node: &EntryNode,
    rect: Rect,
    depth: usize,
    image: &mut RgbaImage,
    hit_map: &mut HitMap,
    hovered_path: Option<&str>,
    selected_path: Option<&str>,
) {
    if rect.w < 1.0 || rect.h < 1.0 || node.size == 0 {
        return;
    }

    let (individual_children, grouped_size) = partition_children(node);

    if individual_children.is_empty() {
        // All contents are too small to show individually — fall back to gradient+hatch leaf.
        draw_folder_leaf(node, rect, depth, image, hit_map, hovered_path, selected_path);
        return;
    }

    // Build the items list: individually-shown children + optional "Other" block for the rest.
    let mut items: Vec<TreemapItem> = individual_children.into_iter().map(TreemapItem::Node).collect();
    if grouped_size > 0 {
        items.push(TreemapItem::Other {
            parent_path: &node.path_str,
            size: grouped_size,
        });
    }

    let node_path = &node.path_str;
    let is_selected = selected_path.is_some_and(|p| p == node_path);
    let is_hovered = hovered_path.is_some_and(|p| p == node_path);

    // Draw folder background (mostly covered by children, visible as thin border gaps).
    let fill = blend(
        [17, 22, 30, 255],
        FileCategory::Folder.color(),
        0.10 + depth as f32 * 0.015,
    );
    fill_rect(image, rect, rgba(fill));

    // Register folder hit FIRST — children drawn on top will override on hover.
    hit_map.regions.push(HitRegion::Rect {
        rect,
        hit: make_hit(node_path, node.size, true, node.dominant_category),
    });

    // Draw children recursively.
    for (item, child_rect) in squarify(items, rect) {
        draw_treemap_item(item, child_rect, depth + 1, image, hit_map, hovered_path, selected_path);
    }

    // Draw selection / hover border on top of children.
    if is_selected {
        stroke_rect(image, rect, rgba([255, 255, 255, 255]));
        stroke_rect(image, rect.inset(1.0), rgba([255, 255, 255, 210]));
    } else if is_hovered {
        stroke_rect(image, rect, rgba([255, 255, 255, 80]));
    }
}

fn draw_treemap_item(
    item: TreemapItem<'_>,
    rect: Rect,
    depth: usize,
    image: &mut RgbaImage,
    hit_map: &mut HitMap,
    hovered_path: Option<&str>,
    selected_path: Option<&str>,
) {
    match item {
        TreemapItem::Node(node) if node.is_dir() => {
            draw_grouped_node(node, rect, depth, image, hit_map, hovered_path, selected_path);
        }
        TreemapItem::Node(node) => draw_file_rect(node, rect, image, hit_map, hovered_path, selected_path),
        TreemapItem::Other { parent_path, size } => draw_other_rect(parent_path, size, rect, image, hit_map),
    }
}

fn draw_other_rect(parent_path: &str, size: u64, rect: Rect, image: &mut RgbaImage, hit_map: &mut HitMap) {
    if rect.area() < 0.5 || size == 0 {
        return;
    }
    let mut fill = shade(FileCategory::Other.color(), 0.80);
    fill = blend(fill, [25, 30, 38, 255], 0.18);
    fill_rect(image, rect, rgba(fill));
    stroke_rect(image, rect, rgba([220, 225, 235, 28]));
    // Register so hover shows tooltip.
    hit_map.regions.push(HitRegion::Rect {
        rect,
        hit: ChartHit {
            path: parent_path.to_string(),
            size,
            is_dir: false,
            category: FileCategory::Other,
            label: "Inne (wiele malych plikow)".to_string(),
        },
    });
}
fn draw_file_rect(
    node: &EntryNode,
    rect: Rect,
    image: &mut RgbaImage,
    hit_map: &mut HitMap,
    hovered_path: Option<&str>,
    selected_path: Option<&str>,
) {
    if rect.area() < 0.5 || node.size == 0 {
        return;
    }
    let node_path = &node.path_str;
    let is_selected = selected_path.is_some_and(|path| path == node_path);
    let is_hovered = hovered_path.is_some_and(|path| path == node_path);
    let mut fill = shade(node.category.color(), 0.92);
    fill = apply_highlight(fill, is_hovered, is_selected);
    fill_rect(image, rect, rgba(fill));
    let border = if is_selected {
        [255, 255, 255, 255]
    } else if is_hovered {
        [255, 255, 255, 170]
    } else {
        [245, 247, 250, 42]
    };
    stroke_rect(image, rect, rgba(border));
    if is_selected {
        stroke_rect(image, rect.inset(1.0), rgba([255, 255, 255, 210]));
    }
    hit_map.regions.push(HitRegion::Rect {
        rect,
        hit: make_hit(node_path, node.size, false, node.category),
    });
}
/// Entry point for treemap layout.
/// Converts children to proportional areas and delegates to the binary-split engine.
fn squarify<'a>(children: Vec<TreemapItem<'a>>, rect: Rect) -> Vec<(TreemapItem<'a>, Rect)> {
    if rect.area() <= 0.0 || children.is_empty() {
        return Vec::new();
    }
    let total_size: u64 = children.iter().map(|c| c.size()).sum();
    if total_size == 0 {
        return Vec::new();
    }
    let canvas_area = rect.area();
    // Compute each item's proportional pixel area.
    let items: Vec<(TreemapItem<'a>, f32)> = children
        .into_iter()
        .map(|c| {
            let area = canvas_area * c.size() as f32 / total_size as f32;
            (c, area)
        })
        .collect();
    let mut out = Vec::with_capacity(items.len() * 10);
    layout_split(&items, rect, &mut out);
    out.shrink_to_fit();
    out
}

/// Recursive binary-split treemap layout.
///
/// Principle: always split the rect along its **longer axis** at the point
/// where the two sub-totals are as equal as possible.  This guarantees that
/// every rectangle stays close to square regardless of item count or size
/// distribution — it never produces the long thin strips that the greedy
/// row-based squarify produces for many equal-sized items.
///
/// Items must arrive pre-sorted by size **descending** (ScanTree ensures this).
fn layout_split<'a>(items: &[(TreemapItem<'a>, f32)], rect: Rect, out: &mut Vec<(TreemapItem<'a>, Rect)>) {
    match items.len() {
        0 => {}
        1 => {
            out.push((items[0].0, rect));
        }
        _ => {
            let total: f32 = items.iter().map(|(_, a)| *a).sum();
            if total <= 0.0 {
                return;
            }
            let target = total * 0.5;

            // Find split index k that brings the cumulative area closest to target.
            // Because items are sorted descending the diff is unimodal → break early.
            let mut cum = 0.0_f32;
            let mut best_k = 1;
            let mut best_diff = f32::INFINITY;
            for k in 1..items.len() {
                cum += items[k - 1].1;
                let diff = (cum - target).abs();
                if diff < best_diff {
                    best_diff = diff;
                    best_k = k;
                } else {
                    break; // diff is now growing → optimal split found
                }
            }

            let area0: f32 = items[..best_k].iter().map(|(_, a)| *a).sum();
            let frac = (area0 / total).clamp(0.01, 0.99);

            // Split along the longer dimension.
            let (r0, r1) = if rect.w >= rect.h {
                let w0 = rect.w * frac;
                (
                    Rect {
                        x: rect.x,
                        y: rect.y,
                        w: w0,
                        h: rect.h,
                    },
                    Rect {
                        x: rect.x + w0,
                        y: rect.y,
                        w: (rect.w - w0).max(0.0),
                        h: rect.h,
                    },
                )
            } else {
                let h0 = rect.h * frac;
                (
                    Rect {
                        x: rect.x,
                        y: rect.y,
                        w: rect.w,
                        h: h0,
                    },
                    Rect {
                        x: rect.x,
                        y: rect.y + h0,
                        w: rect.w,
                        h: (rect.h - h0).max(0.0),
                    },
                )
            };

            layout_split(&items[..best_k], r0, out);
            layout_split(&items[best_k..], r1, out);
        }
    }
}
impl TreemapItem<'_> {
    fn size(self) -> u64 {
        match self {
            Self::Node(node) => node.size,
            Self::Other { size, .. } => size,
        }
    }
}

fn partition_children(node: &EntryNode) -> (Vec<&EntryNode>, u64) {
    partition_entries(node.visible_children(), node.size)
}

fn partition_entries<'a>(entries: impl Iterator<Item = &'a EntryNode>, total_size: u64) -> (Vec<&'a EntryNode>, u64) {
    // Children arrive in size-descending order (pre-sorted by ScanTree::recompute).
    // No sort needed here.
    let mut children: Vec<&'a EntryNode> = entries.collect();
    if total_size == 0 || children.is_empty() {
        return (children, 0);
    }
    // Use partition_point for O(log n) threshold split.
    let threshold = ((total_size as f64) * CHILD_GROUP_RATIO) as u64;
    let split = if threshold == 0 {
        children.len()
    } else {
        children.partition_point(|c| c.size > threshold)
    };
    let grouped_size: u64 = children[split..].iter().map(|c| c.size).sum();
    let grouped_size = if grouped_size > 0 {
        let grouped_ratio = grouped_size as f64 / total_size as f64;
        if grouped_ratio < MIN_OTHER_RATIO {
            0
        } else {
            grouped_size
        }
    } else {
        0
    };
    children.truncate(split);
    (children, grouped_size)
}

fn find_node<'a>(node: &'a EntryNode, path: &str) -> Option<&'a EntryNode> {
    if node.path_str == path {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_node(child, path) {
            return Some(found);
        }
    }
    None
}

pub fn find_node_in_tree<'a>(tree: &'a ScanTree, path: &str) -> Option<&'a EntryNode> {
    for root in &tree.roots {
        if let Some(found) = find_node(root, path) {
            return Some(found);
        }
    }
    None
}

/// Return treemap items for the given view_path, or root-level items if not found.
fn view_root_items<'a>(tree: &'a ScanTree, view_path: Option<&str>) -> Vec<TreemapItem<'a>> {
    if let Some(vp) = view_path
        && let Some(node) = find_node_in_tree(tree, vp)
    {
        return treemap_items(node);
    }
    treemap_root_items(tree)
}

fn treemap_items<'a>(node: &'a EntryNode) -> Vec<TreemapItem<'a>> {
    let (children, grouped_size) = partition_children(node);
    let mut items = children.into_iter().map(TreemapItem::Node).collect::<Vec<_>>();
    if grouped_size > 0 {
        items.push(TreemapItem::Other {
            parent_path: &node.path_str,
            size: grouped_size,
        });
    }
    items
}

fn treemap_root_items<'a>(tree: &'a ScanTree) -> Vec<TreemapItem<'a>> {
    let (children, grouped_size) = partition_entries(tree.roots.iter().filter(|entry| entry.size > 0), tree.total_size);
    let mut items = children.into_iter().map(TreemapItem::Node).collect::<Vec<_>>();
    if grouped_size > 0 {
        items.push(TreemapItem::Other {
            parent_path: "/",
            size: grouped_size,
        });
    }
    items
}
fn make_hit(path: &str, size: u64, is_dir: bool, category: FileCategory) -> ChartHit {
    let display = display_name(Path::new(path));
    let label = if is_dir {
        format!("{}: {}", kind_label(&NodeKind::Directory), display)
    } else {
        format!("{}: {}", kind_label(&NodeKind::File), display)
    };
    ChartHit {
        path: path.to_string(),
        size,
        is_dir,
        category,
        label,
    }
}
fn fill_rect(image: &mut RgbaImage, rect: Rect, color: Rgba<u8>) {
    let x_start = rect.x.max(0.0).floor() as u32;
    let y_start = rect.y.max(0.0).floor() as u32;
    let x_end = (rect.x + rect.w).min(image.width() as f32).ceil() as u32;
    let y_end = (rect.y + rect.h).min(image.height() as f32).ceil() as u32;
    for y in y_start..y_end {
        for x in x_start..x_end {
            image.put_pixel(x, y, color);
        }
    }
}
fn stroke_rect(image: &mut RgbaImage, rect: Rect, color: Rgba<u8>) {
    let x_start = rect.x.max(0.0).floor() as u32;
    let y_start = rect.y.max(0.0).floor() as u32;
    let x_end = (rect.x + rect.w).min(image.width() as f32).ceil() as u32;
    let y_end = (rect.y + rect.h).min(image.height() as f32).ceil() as u32;
    if x_start >= x_end || y_start >= y_end {
        return;
    }
    for x in x_start..x_end {
        image.put_pixel(x, y_start, color);
        image.put_pixel(x, y_end.saturating_sub(1), color);
    }
    for y in y_start..y_end {
        image.put_pixel(x_start, y, color);
        image.put_pixel(x_end.saturating_sub(1), y, color);
    }
}
fn blend(base: [u8; 4], overlay: [u8; 4], amount: f32) -> [u8; 4] {
    let amount = amount.clamp(0.0, 1.0);
    [
        (base[0] as f32 * (1.0 - amount) + overlay[0] as f32 * amount) as u8,
        (base[1] as f32 * (1.0 - amount) + overlay[1] as f32 * amount) as u8,
        (base[2] as f32 * (1.0 - amount) + overlay[2] as f32 * amount) as u8,
        255,
    ]
}
fn shade(color: [u8; 4], multiplier: f32) -> [u8; 4] {
    let multiplier = multiplier.max(0.0);
    [
        (color[0] as f32 * multiplier).clamp(0.0, 255.0) as u8,
        (color[1] as f32 * multiplier).clamp(0.0, 255.0) as u8,
        (color[2] as f32 * multiplier).clamp(0.0, 255.0) as u8,
        color[3],
    ]
}
fn apply_highlight(color: [u8; 4], hovered: bool, selected: bool) -> [u8; 4] {
    if selected {
        return blend(color, [255, 255, 255, 255], 0.30);
    }
    if hovered {
        return blend(color, [255, 255, 255, 255], 0.14);
    }
    color
}
fn rgba(color: [u8; 4]) -> Rgba<u8> {
    Rgba(color)
}
fn to_slint_image(image: &RgbaImage) -> Image {
    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(image.as_raw(), image.width(), image.height());
    Image::from_rgba8(buffer)
}
