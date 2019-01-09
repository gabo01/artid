use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

macro_rules! run {
    ($folder:ident, $options:ident , $op:ident) => {{
        let model = <FileSystemFolder as Operator<$op>>::modelate(&mut $folder, $options)
            .expect("Unable to build the model");

        model.run().expect("Unable to run the model");
    }};
}

#[derive(Clone, Debug)]
pub enum FileKind {
    File(String),
    Dir,
    Symlink,
    Deleted,
}

pub struct FileTree<P: AsRef<Path>> {
    root: P,
    files: Vec<(String, FileKind)>,
}

impl<P: AsRef<Path>> FileTree<P> {
    pub fn new(root: P) -> Self {
        Self {
            root,
            files: vec![],
        }
    }

    pub fn add_file<S: Into<String>>(&mut self, file: S) {
        let file = file.into();
        create_file!(self.path().join(&file), "{}", &file);
        self.files.push((file.clone(), FileKind::File(file)));
    }

    pub fn add_dir<S: Into<String>>(&mut self, dir: S) {
        let dir = dir.into();
        fs::create_dir_all(self.path().join(&dir)).expect("Unable to create dir");
        self.files.push((dir, FileKind::Dir));
    }

    pub fn add_root(&mut self) {
        if !self.path().exists() {
            self.add_dir("");
        }
    }

    pub fn add_symlink<S: Into<String>, T: AsRef<Path>>(&mut self, file: S, link: T) {
        #[cfg(unix)]
        use std::os::unix::fs::symlink;
        #[cfg(windows)]
        use std::os::windows::fs::symlink_file as symlink;

        let file = file.into();
        let link = link.as_ref();
        symlink(link, self.path().join(&file)).expect("Unable to create the link");
        self.files.push((file, FileKind::Symlink));
    }

    pub fn copy_tree<T: AsRef<Path>>(&mut self, tree: &FileTree<T>) {
        self.files.append(&mut tree.files.clone());
    }

    pub fn transform<S: Into<String>>(&mut self, file: S, kind: FileKind) {
        let file = file.into();
        self.files.iter_mut().find(|e| e.0 == file).unwrap().1 = kind;
    }

    pub fn modify<S: Into<String>>(&mut self, file: S, contents: &str) {
        let file = file.into();
        self.files.iter_mut().find(|e| e.0 == file).unwrap().1 = FileKind::File(contents.into());

        let mut pointer = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(self.path().join(&file))
            .expect("Unable to open file");
        write!(pointer, "{}", contents).unwrap();
    }

    pub fn remove<S: Into<String>>(&mut self, file: S) {
        let file = file.into();
        self.files.iter_mut().find(|e| e.0 == file).unwrap().1 = FileKind::Deleted;
        fs::remove_file(self.path().join(file)).expect("Unable to delete file");
    }

    pub fn assert(&self) {
        let root = self.path();
        assert!(root.exists());

        for file in &self.files {
            match file.1 {
                FileKind::File(ref contents) => {
                    let path = root.join(&file.0);
                    assert!(path.exists());
                    assert!(filetype!(&path).is_file());
                    assert_eq!(read_file!(path), *contents);
                }

                FileKind::Symlink => {
                    assert!(root.join(&file.0).exists());
                    assert!(symlink!(root.join(&file.0)));
                }

                FileKind::Dir => assert!(root.join(&file.0).exists()),
                FileKind::Deleted => assert!(!root.join(&file.0).exists()),
            }
        }
    }

    pub fn path(&self) -> &Path {
        self.root.as_ref()
    }

    pub fn generate_from(path: P) -> FileTree<P> {
        let mut root = FileTree::new(path);
        root.add_root();
        root.add_file("a.txt");
        root.add_file("b.txt");
        root
    }
}

impl FileTree<TempDir> {
    pub fn create() -> FileTree<TempDir> {
        FileTree::new(tmpdir!())
    }

    pub fn generate() -> FileTree<TempDir> {
        FileTree::generate_from(tmpdir!())
    }
}

impl<P: AsRef<Path>> From<P> for FileTree<P> {
    fn from(path: P) -> Self {
        FileTree::new(path)
    }
}
