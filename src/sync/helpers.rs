use std::path::Path;

/// Represents the different types a path can take on the file system. It is just a convenience
/// enum for using a match instead of an if-else tree.
#[derive(Debug, PartialEq)]
pub enum FileSystemType {
    File,
    Dir,
    Other,
}

impl<P: AsRef<Path>> From<P> for FileSystemType {
    fn from(path: P) -> Self {
        let path = path.as_ref();
        if path.is_file() {
            FileSystemType::File
        } else if path.is_dir() {
            FileSystemType::Dir
        } else {
            FileSystemType::Other
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{check, DirRoot, FileSystemType};
    use std::cell::RefCell;
    use tempfile;

    mod file_system {
        use super::FileSystemType;
        use std::fs::File;
        use tempfile;

        #[test]
        fn test_system_dir() {
            let dir = tmpdir!();
            assert_eq!(FileSystemType::from(dir.path()), FileSystemType::Dir);
        }

        #[test]
        fn test_system_file() {
            let dir = tmpdir!();
            let path = create_file!(tmppath!(dir, "a.txt"));
            assert_eq!(FileSystemType::from(path), FileSystemType::File);
        }
    }
}
