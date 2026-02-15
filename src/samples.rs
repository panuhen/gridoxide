use std::path::{Path, PathBuf};

/// Entry for a discovered sample file
pub struct SampleEntry {
    pub path: PathBuf,      // absolute path
    pub relative: String,   // display path (relative to search root)
    pub name: String,       // filename without extension
    pub dir: String,        // parent folder name (e.g. "kicks")
}

/// Get the global samples directory (~/.gridoxide/samples/)
pub fn samples_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".gridoxide").join("samples")
}

/// Create the samples directory structure if it doesn't exist
pub fn ensure_samples_dir() {
    let base = samples_dir();
    let subdirs = ["kicks", "snares", "hihats", "bass", "perc", "loops", "other"];
    for subdir in &subdirs {
        let dir = base.join(subdir);
        if !dir.exists() {
            let _ = std::fs::create_dir_all(&dir);
        }
    }
}

/// Get the default search directories for samples
pub fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    // Project-local samples/ first
    let local = PathBuf::from("./samples");
    if local.is_dir() {
        dirs.push(local);
    }
    // Global samples directory
    let global = samples_dir();
    if global.is_dir() {
        dirs.push(global);
    }
    dirs
}

/// Scan directories recursively for .wav files
pub fn scan_samples(dirs: &[PathBuf]) -> Vec<SampleEntry> {
    let mut entries = Vec::new();
    for dir in dirs {
        scan_dir(dir, dir, &mut entries);
    }
    // Sort by directory then name
    entries.sort_by(|a, b| a.relative.cmp(&b.relative));
    entries
}

fn scan_dir(root: &Path, current: &Path, entries: &mut Vec<SampleEntry>) {
    let Ok(read_dir) = std::fs::read_dir(current) else {
        return;
    };

    let mut items: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
    items.sort_by_key(|e| e.file_name());

    for entry in items {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(root, &path, entries);
        } else if path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("wav"))
            .unwrap_or(false)
        {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let dir = path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            entries.push(SampleEntry {
                path: path.canonicalize().unwrap_or(path),
                relative,
                name,
                dir,
            });
        }
    }
}

/// Resolve a sample name/path to an absolute path
/// Searches project-local ./samples/ first, then global ~/.gridoxide/samples/
/// Also accepts absolute paths directly
pub fn resolve_sample_path(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let as_path = Path::new(name);

    // If it's already an absolute path and exists, use it
    if as_path.is_absolute() && as_path.exists() {
        return Some(as_path.to_path_buf());
    }

    // Search in each directory
    for dir in dirs {
        let full = dir.join(name);
        if full.exists() {
            return Some(full);
        }
    }

    None
}
