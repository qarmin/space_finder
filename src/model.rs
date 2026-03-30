use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    path::{Path, PathBuf},
};
pub const CATEGORY_COUNT: usize = 10;
const TOP_ENTRIES_CACHE_LIMIT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileCategory {
    Folder,
    Audio,
    Video,
    Image,
    Archive,
    Document,
    Code,
    DiskImage,
    Binary,
    Other,
}
impl FileCategory {
    pub const ALL: [Self; CATEGORY_COUNT] = [
        Self::Folder,
        Self::Audio,
        Self::Video,
        Self::Image,
        Self::Archive,
        Self::Document,
        Self::Code,
        Self::DiskImage,
        Self::Binary,
        Self::Other,
    ];
    pub fn index(self) -> usize {
        match self {
            Self::Folder => 0,
            Self::Audio => 1,
            Self::Video => 2,
            Self::Image => 3,
            Self::Archive => 4,
            Self::Document => 5,
            Self::Code => 6,
            Self::DiskImage => 7,
            Self::Binary => 8,
            Self::Other => 9,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Folder => "Foldery",
            Self::Audio => "Muzyka",
            Self::Video => "Filmy",
            Self::Image => "Obrazy",
            Self::Archive => "Archiwa",
            Self::Document => "Dokumenty",
            Self::Code => "Kod",
            Self::DiskImage => "Obrazy dysków",
            Self::Binary => "Binaria",
            Self::Other => "Inne",
        }
    }
    pub fn color(self) -> [u8; 4] {
        match self {
            Self::Folder => [105, 115, 135, 255],
            Self::Audio => [240, 202, 87, 255],
            Self::Video => [228, 87, 88, 255],
            Self::Image => [174, 110, 247, 255],
            Self::Archive => [224, 151, 78, 255],
            Self::Document => [84, 160, 255, 255],
            Self::Code => [72, 201, 176, 255],
            Self::DiskImage => [76, 201, 240, 255],
            Self::Binary => [120, 126, 255, 255],
            Self::Other => [141, 153, 174, 255],
        }
    }
    pub fn slint_color(self) -> slint::Color {
        let [r, g, b, _] = self.color();
        slint::Color::from_rgb_u8(r, g, b)
    }
    pub fn from_path(path: &Path) -> Self {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        match extension.as_str() {
            "mp3" | "flac" | "wav" | "aac" | "ogg" | "opus" | "m4a" | "wma" | "aiff" | "alac" => Self::Audio,
            "mp4" | "mkv" | "avi" | "mov" | "webm" | "m4v" | "wmv" | "mpg" | "mpeg" | "ts" | "flv" => Self::Video,
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tiff" | "svg" | "heic" | "avif" | "raw" => Self::Image,
            "zip" | "7z" | "rar" | "tar" | "gz" | "xz" | "bz2" | "zst" | "cab" | "isoz" => Self::Archive,
            "pdf" | "doc" | "docx" | "odt" | "rtf" | "txt" | "md" | "xls" | "xlsx" | "ppt" | "pptx" | "epub" => {
                Self::Document
            }
            "rs" | "c" | "h" | "hpp" | "cpp" | "cc" | "py" | "js" | "tsx" | "jsx" | "java" | "kt" | "go" | "php"
            | "rb" | "swift" | "cs" | "toml" | "json" | "yaml" | "yml" | "xml" | "html" | "css" | "scss" => Self::Code,
            "iso" | "img" | "dmg" | "vmdk" | "qcow2" | "vdi" => Self::DiskImage,
            "exe" | "dll" | "so" | "bin" | "appimage" | "deb" | "rpm" | "msi" => Self::Binary,
            _ => Self::Other,
        }
    }

    pub fn from_mime(mime: &str) -> Option<Self> {
        let mime = mime.to_ascii_lowercase();

        if mime.starts_with("audio/") {
            Some(Self::Audio)
        } else if mime.starts_with("video/") {
            Some(Self::Video)
        } else if mime.starts_with("image/") {
            Some(Self::Image)
        } else if mime.contains("zip")
            || mime.contains("tar")
            || mime.contains("rar")
            || mime.contains("7z")
            || mime.contains("archive")
            || mime.contains("compressed")
        {
            Some(Self::Archive)
        } else if mime.contains("pdf")
            || mime.contains("document")
            || mime.contains("sheet")
            || mime.contains("presentation")
            || mime.starts_with("text/")
        {
            Some(Self::Document)
        } else if mime.contains("elf")
            || mime.contains("executable")
            || mime.contains("sharedlib")
            || mime.contains("mach-binary")
            || mime.contains("pe32")
        {
            Some(Self::Binary)
        } else {
            None
        }
    }
}

pub fn detect_file_category(path: &Path) -> FileCategory {
    if let Ok(Some(kind)) = infer::get_from_path(path)
        && let Some(category) = FileCategory::from_mime(kind.mime_type())
    {
        return category;
    }

    FileCategory::from_path(path)
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    File,
    Directory,
    Symlink,
    Inaccessible,
}
#[derive(Debug, Clone)]
pub struct EntryNode {
    pub name: String,
    pub path: PathBuf,
    pub path_str: String,
    pub size: u64,
    pub kind: NodeKind,
    pub category: FileCategory,
    pub dominant_category: FileCategory,
    pub children: Vec<Self>,
    pub depth: usize,
    /// Accumulated file-size per category for all descendants (index = FileCategory::index()).
    /// Always 0 for the Folder category (index 0) since files are never categorized as Folder.
    pub category_weights: [u64; CATEGORY_COUNT],
}
impl EntryNode {
    pub fn file(path: PathBuf, size: u64, depth: usize) -> Self {
        let category = detect_file_category(&path);
        let mut category_weights = [0_u64; CATEGORY_COUNT];
        category_weights[category.index()] = size;
        Self {
            name: display_name(&path),
            path_str: path.to_string_lossy().into_owned(),
            path,
            size,
            kind: NodeKind::File,
            category,
            dominant_category: category,
            children: Vec::new(),
            depth,
            category_weights,
        }
    }
    pub fn symlink(path: PathBuf, depth: usize) -> Self {
        Self {
            name: display_name(&path),
            path_str: path.to_string_lossy().into_owned(),
            path,
            size: 0,
            kind: NodeKind::Symlink,
            category: FileCategory::Other,
            dominant_category: FileCategory::Other,
            children: Vec::new(),
            depth,
            category_weights: [0; CATEGORY_COUNT],
        }
    }
    pub fn inaccessible(path: PathBuf, depth: usize) -> Self {
        Self {
            name: display_name(&path),
            path_str: path.to_string_lossy().into_owned(),
            path,
            size: 0,
            kind: NodeKind::Inaccessible,
            category: FileCategory::Other,
            dominant_category: FileCategory::Other,
            children: Vec::new(),
            depth,
            category_weights: [0; CATEGORY_COUNT],
        }
    }
    pub fn directory(path: PathBuf, children: Vec<Self>, depth: usize) -> Self {
        let mut node = Self {
            name: display_name(&path),
            path_str: path.to_string_lossy().into_owned(),
            path,
            size: 0,
            kind: NodeKind::Directory,
            category: FileCategory::Other,
            dominant_category: FileCategory::Other,
            children,
            depth,
            category_weights: [0; CATEGORY_COUNT],
        };
        node.refresh_categories();
        node
    }
    pub fn is_dir(&self) -> bool {
        matches!(self.kind, NodeKind::Directory)
    }
    pub fn visible_children(&self) -> impl Iterator<Item = &Self> {
        self.children.iter().filter(|child| child.size > 0)
    }
    /// Sort children by size descending (then path ascending) recursively.
    /// Called once after recompute so render code can skip sort.
    pub fn sort_children_by_size(&mut self) {
        self.children
            .sort_unstable_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
        for child in &mut self.children {
            child.sort_children_by_size();
        }
    }
    pub fn refresh_categories(&mut self) -> [u64; CATEGORY_COUNT] {
        match self.kind {
            NodeKind::File => {
                let mut weights = [0_u64; CATEGORY_COUNT];
                weights[self.category.index()] = self.size;
                self.dominant_category = self.category;
                self.category_weights = weights;
                weights
            }
            NodeKind::Directory => {
                self.size = 0;
                let mut weights = [0_u64; CATEGORY_COUNT];
                for child in &mut self.children {
                    let child_weights = child.refresh_categories();
                    self.size = self.size.saturating_add(child.size);
                    for (idx, value) in child_weights.into_iter().enumerate() {
                        weights[idx] = weights[idx].saturating_add(value);
                    }
                }
                let dominant_idx = weights
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, value)| **value)
                    .map_or(FileCategory::Other.index(), |(idx, _)| idx);
                self.dominant_category = FileCategory::ALL[dominant_idx];
                self.category_weights = weights;
                weights
            }
            NodeKind::Symlink | NodeKind::Inaccessible => {
                self.size = 0;
                self.category = FileCategory::Other;
                self.dominant_category = FileCategory::Other;
                self.category_weights = [0; CATEGORY_COUNT];
                [0_u64; CATEGORY_COUNT]
            }
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct ScanTree {
    pub roots: Vec<EntryNode>,
    pub total_size: u64,
    pub file_count: u64,
    pub dir_count: u64,
    pub warnings: u64,
    pub scanned_entries: u64,
    pub canceled: bool,
    #[doc(hidden)]
    pub top_entries_cache: Vec<TopEntry>,
}
impl ScanTree {
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty() || self.total_size == 0
    }
    pub fn recompute(&mut self) {
        self.total_size = 0;
        for root in &mut self.roots {
            root.refresh_categories();
            root.sort_children_by_size(); // pre-sort so render never needs to sort
            self.total_size = self.total_size.saturating_add(root.size);
        }
        self.roots
            .sort_unstable_by(|left, right| right.size.cmp(&left.size).then_with(|| left.path.cmp(&right.path)));
        self.top_entries_cache = compute_top_entries(&self.roots, TOP_ENTRIES_CACHE_LIMIT);
    }
    pub fn virtual_root(&self) -> EntryNode {
        let mut root = EntryNode {
            name: "Wszystkie ścieżki".into(),
            path: PathBuf::from("/"),
            path_str: "/".to_string(),
            size: self.total_size,
            kind: NodeKind::Directory,
            category: FileCategory::Other,
            dominant_category: FileCategory::Other,
            children: self.roots.clone(),
            depth: 0,
            category_weights: [0; CATEGORY_COUNT],
        };
        root.refresh_categories();
        root
    }
    pub fn top_entries(&self, limit: usize) -> Vec<TopEntry> {
        if limit == 0 {
            return Vec::new();
        }
        if limit <= self.top_entries_cache.len() {
            return self.top_entries_cache[..limit].to_vec();
        }
        compute_top_entries(&self.roots, limit)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopEntry {
    pub path: String,
    pub size: u64,
    pub kind: String,
    pub category: FileCategory,
}
#[derive(Debug, Clone, Copy)]
struct TopFileRef<'a> {
    path: &'a Path,
    size: u64,
    category: FileCategory,
}

#[derive(Debug, Clone, Copy)]
struct WorstFirst<'a>(TopFileRef<'a>);

impl PartialEq for WorstFirst<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.0.size == other.0.size && self.0.path == other.0.path
    }
}

impl Eq for WorstFirst<'_> {}

impl PartialOrd for WorstFirst<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WorstFirst<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Keep the weakest element as heap top: smaller size, and for ties lexicographically larger path.
        other
            .0
            .size
            .cmp(&self.0.size)
            .then_with(|| self.0.path.cmp(other.0.path))
    }
}

fn compute_top_entries(roots: &[EntryNode], limit: usize) -> Vec<TopEntry> {
    if limit == 0 {
        return Vec::new();
    }
    let mut heap = BinaryHeap::with_capacity(limit);
    for root in roots {
        collect_top_files(root, limit, &mut heap);
    }
    let mut ranked = heap
        .into_iter()
        .map(|entry| TopEntry {
            path: entry.0.path.to_string_lossy().to_string(),
            size: entry.0.size,
            kind: "Plik".to_string(),
            category: entry.0.category,
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.size.cmp(&left.size).then_with(|| left.path.cmp(&right.path)));
    ranked
}

fn collect_top_files<'a>(node: &'a EntryNode, limit: usize, heap: &mut BinaryHeap<WorstFirst<'a>>) {
    if matches!(node.kind, NodeKind::File) && node.size > 0 {
        let candidate = TopFileRef {
            path: &node.path,
            size: node.size,
            category: node.category,
        };
        if heap.len() < limit {
            heap.push(WorstFirst(candidate));
        } else if let Some(worst) = heap.peek().map(|entry| entry.0)
            && (candidate.size > worst.size || (candidate.size == worst.size && candidate.path < worst.path))
        {
            heap.pop();
            heap.push(WorstFirst(candidate));
        }
    }
    for child in &node.children {
        collect_top_files(child, limit, heap);
    }
}
pub fn kind_label(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::File => "Plik",
        NodeKind::Directory => "Folder",
        NodeKind::Symlink => "Symlink",
        NodeKind::Inaccessible => "Brak dostępu",
    }
}
pub fn display_name(path: &Path) -> String {
    path.file_name()
        .or_else(|| path.components().next_back().map(|component| component.as_os_str()))
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit_idx = 0;
    while value >= 1024.0 && unit_idx < UNITS.len() - 1 {
        value /= 1024.0;
        unit_idx += 1;
    }
    format!("{value:.2} {}", UNITS[unit_idx])
}
pub fn merge_paths(existing: &mut Vec<PathBuf>, new_paths: impl IntoIterator<Item = PathBuf>) {
    for path in new_paths {
        if !existing.iter().any(|item| item == &path) {
            existing.push(path);
        }
    }
    existing.sort();
}
pub fn detect_path_kind(path: &Path) -> &'static str {
    if path.is_dir() {
        "Folder"
    } else if path.is_file() {
        "Plik"
    } else {
        "Ścieżka"
    }
}
