//! File path completion for interactive prompts
//!
//! Provides tab completion for file paths in dialoguer Input prompts.

use dialoguer::Completion;
use std::path::{Path, PathBuf};

/// File path completer for interactive input
pub struct FilePathCompleter {
    /// Current working directory for relative path resolution
    cwd: PathBuf,
}

impl FilePathCompleter {
    /// Create a new file path completer with the current working directory
    pub fn new() -> Self {
        Self::from_cwd(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    /// Create a new file path completer with an explicit working directory
    pub fn from_cwd(cwd: impl Into<PathBuf>) -> Self {
        Self { cwd: cwd.into() }
    }

    /// Get all file entries in a directory that match a prefix
    fn get_matching_entries(&self, dir: &Path, prefix: &str) -> Vec<String> {
        let mut matches = Vec::new();

        // Read directory entries
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();

                // Check if this entry matches the prefix
                if prefix.is_empty() || name.starts_with(prefix) {
                    let mut path = name.to_string();

                    // Add trailing slash for directories
                    if entry.path().is_dir() {
                        path.push('/');
                    }

                    matches.push(path);
                }
            }
        }

        // Sort matches alphabetically (directories first)
        matches.sort_by(|a, b| {
            let a_is_dir = a.ends_with('/');
            let b_is_dir = b.ends_with('/');

            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.cmp(b),
            }
        });

        matches
    }
}

impl Completion for FilePathCompleter {
    /// Get completion for the current input
    fn get(&self, input: &str) -> Option<String> {
        // Handle empty input - show current directory contents
        if input.is_empty() {
            let matches = self.get_matching_entries(&self.cwd, "");
            return matches.first().map(|s| s.to_string());
        }

        // Parse input path
        let input_path = Path::new(input);

        // Determine base directory and file prefix
        let (base_dir, file_prefix) = if input.ends_with('/') {
            // Input is a directory path
            (self.cwd.join(input_path), "")
        } else if let Some(parent) = input_path.parent() {
            // Input has a parent directory
            let file_name = input_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            if parent.as_os_str().is_empty() {
                (self.cwd.clone(), file_name)
            } else {
                (self.cwd.join(parent), file_name)
            }
        } else {
            // Input is just a filename in current directory
            (self.cwd.clone(), input)
        };

        // Get matching entries
        let matches = self.get_matching_entries(&base_dir, file_prefix);

        // Return first match if any
        matches.first().map(|matched_name| {
            // Construct full path
            if let Some(parent) = input_path.parent() {
                if !parent.as_os_str().is_empty() {
                    format!("{}/{}", parent.display(), matched_name)
                } else {
                    matched_name.to_string()
                }
            } else {
                matched_name.to_string()
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_completion_in_temp_dir() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create some test files and directories
        fs::write(temp_path.join("file1.txt"), "").unwrap();
        fs::write(temp_path.join("file2.txt"), "").unwrap();
        fs::create_dir(temp_path.join("subdir")).unwrap();
        fs::write(temp_path.join("subdir/nested.txt"), "").unwrap();

        let completer = FilePathCompleter::from_cwd(temp_path.to_path_buf());

        // Test: empty input should return first entry
        let result = completer.get("");
        assert!(result.is_some());

        // Test: prefix matching
        let result = completer.get("file");
        assert!(result.is_some());
        let matched = result.unwrap();
        assert!(matched.starts_with("file"));

        // Test: directory completion
        let result = completer.get("sub");
        assert_eq!(result, Some("subdir/".to_string()));
    }
}
