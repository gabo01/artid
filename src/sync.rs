use failure::ResultExt;
use logger::pathlight;
use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::fs;
use std::iter::FromIterator;
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_file as symlink;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use {FsError, Result};

///
macro_rules! read {
    ($path:expr) => {
        fs::read_dir($path)
            .context(FsError::ReadFile($path.into()))?
            .into_iter()
            .filter_map(|e| e.ok())
            .map(|e| (e.path(), e.file_name()))
    };
}

#[derive(Debug)]
pub struct DirTree<'a> {
    src: &'a Path,
    dst: &'a Path,
    root: TreeNode,
}

impl<'a> DirTree<'a> {
    pub fn new(src: &'a Path, dst: &'a Path) -> Result<Self> {
        let (srcexists, dstexists) = (src.exists(), dst.exists());
        let presence = if srcexists && dstexists {
            Presence::Both
        } else if srcexists {
            Presence::Src
        } else {
            Presence::Dst
        };

        let mut root = TreeNode::new("".into(), presence, FileType::Dir);
        root.read_recursive(&src, &dst)?;

        Ok(Self { src, dst, root })
    }

    pub fn iter<'b>(&'b self) -> TreeIter<'a, 'b> {
        TreeIter::new(self)
    }
}

impl<'a, 'b> IntoIterator for &'b DirTree<'a>
where
    'a: 'b,
{
    type Item = TreeIterNode<'a, 'b>;
    type IntoIter = TreeIter<'a, 'b>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Presence {
    Src,
    Dst,
    Both,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FileType {
    File,
    Dir,
    Other,
}

impl<P: AsRef<Path>> From<P> for FileType {
    fn from(path: P) -> Self {
        let path = path.as_ref();
        if path.is_file() {
            FileType::File
        } else if path.is_dir() {
            FileType::Dir
        } else {
            FileType::Other
        }
    }
}

#[derive(Debug)]
pub struct TreeNode {
    path: PathBuf,
    presence: Presence,
    kind: FileType,
    children: Option<Vec<TreeNode>>,
}

impl TreeNode {
    pub fn new(path: PathBuf, presence: Presence, kind: FileType) -> Self {
        Self {
            path,
            presence,
            kind,
            children: None,
        }
    }

    pub fn read_recursive<T, P>(&mut self, src: T, dst: P) -> Result<()>
    where
        T: AsRef<Path>,
        P: AsRef<Path>,
    {
        self.read(src.as_ref(), dst.as_ref())?;

        if let Some(ref mut val) = self.children {
            for child in val {
                if child.kind == FileType::Dir {
                    child.read_recursive(src.as_ref(), dst.as_ref())?;
                }
            }
        } else {
            unreachable!();
        }

        Ok(())
    }

    pub fn read<T: AsRef<Path>, P: AsRef<Path>>(&mut self, src: T, dst: P) -> Result<()> {
        let src = src.as_ref().join(&self.path);
        let dst = dst.as_ref().join(&self.path);

        match self.presence {
            Presence::Both => {
                self.children = Some(Self::compare(&self.path, read!(&src), read!(&dst))?);
            }

            Presence::Src => {
                self.children = Some(
                    read!(&src)
                        .map(|(path, name)| {
                            TreeNode::new(self.path.join(name), Presence::Src, FileType::from(path))
                        }).collect(),
                );
            }

            Presence::Dst => {
                self.children = Some(
                    read!(&dst)
                        .map(|(path, name)| {
                            TreeNode::new(self.path.join(name), Presence::Dst, FileType::from(path))
                        }).collect(),
                );
            }
        }

        Ok(())
    }

    fn compare<P, T, U>(path: P, src: T, dst: U) -> Result<Vec<TreeNode>>
    where
        P: AsRef<Path>,
        T: Iterator<Item = (PathBuf, OsString)>,
        U: Iterator<Item = (PathBuf, OsString)>,
    {
        let mut table = HashMap::new();
        let path = path.as_ref();

        for entry in src {
            table.insert(entry.1, (entry.0, Presence::Src));
        }

        for entry in dst {
            let (path, name) = entry;
            table
                .entry(name)
                .and_modify(|val| *val = (path.clone(), Presence::Both))
                .or_insert((path, Presence::Dst));
        }

        let vec = table
            .drain()
            .map(|(key, value)| TreeNode::new(path.join(key), value.1, FileType::from(value.0)))
            .collect();

        Ok(vec)
    }
}

#[derive(Debug)]
pub struct TreeIter<'a, 'b>
where
    'a: 'b,
{
    tree: &'b DirTree<'a>,
    elements: VecDeque<TreeIterNode<'a, 'b>>,
}

impl<'a, 'b> TreeIter<'a, 'b>
where
    'a: 'b,
{
    pub fn new(tree: &'b DirTree<'a>) -> Self {
        let mut elements = VecDeque::new();
        elements.push_back(TreeIterNode::new(&tree.src, &tree.dst, &tree.root));

        Self { tree, elements }
    }
}

impl<'a, 'b> Iterator for TreeIter<'a, 'b>
where
    'a: 'b,
{
    type Item = TreeIterNode<'a, 'b>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.elements.pop_front();

        match next {
            Some(val) => {
                if val.node.kind == FileType::Dir {
                    if let Some(ref children) = val.node.children {
                        self.elements.extend(
                            children
                                .iter()
                                .map(|e| TreeIterNode::new(val.src, val.dst, e)),
                        );
                    }
                }

                Some(val)
            }

            None => None,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Direction {
    Forward,
    Backward,
}

#[derive(Debug)]
pub struct TreeIterNode<'a, 'b> {
    pub src: &'a Path,
    pub dst: &'a Path,
    pub node: &'b TreeNode,
}

impl<'a, 'b> TreeIterNode<'a, 'b> {
    pub fn new(src: &'a Path, dst: &'a Path, node: &'b TreeNode) -> Self {
        Self { src, dst, node }
    }

    pub fn presence(&self) -> Presence {
        self.node.presence
    }

    pub fn kind(&self) -> FileType {
        self.node.kind
    }

    pub fn path(&self) -> &Path {
        &self.node.path
    }

    pub fn synced(&self, direction: Direction) -> bool {
        let (to_sync, to_be_synced) = match direction {
            Direction::Forward => (
                self.src.join(&self.node.path),
                self.dst.join(&self.node.path),
            ),

            Direction::Backward => (
                self.dst.join(&self.node.path),
                self.src.join(&self.node.path),
            ),
        };

        if to_sync.exists() && to_be_synced.exists() {
            if let Some(src) = modified(&to_sync) {
                if let Some(dst) = modified(&to_be_synced) {
                    return dst >= src;
                }
            }
        }

        false
    }
}

pub enum CopyAction {
    CreateDir { target: PathBuf },
    CopyFile { src: PathBuf, dst: PathBuf },
    CopyLink { src: PathBuf, dst: PathBuf },
}

pub struct CopyModel {
    actions: Vec<CopyAction>,
}

impl CopyModel {
    pub fn new(actions: Vec<CopyAction>) -> Self {
        Self { actions }
    }

    pub fn execute(self) -> Result<()> {
        for action in self.actions {
            match action {
                CopyAction::CreateDir { target } => {
                    if !target.exists() {
                        fs::create_dir(&target).context(FsError::OpenFile((&target).into()))?;
                    }
                }

                CopyAction::CopyFile { src, dst } => {
                    if let Ok(metadata) = fs::symlink_metadata(&dst) {
                        if metadata.file_type().is_symlink() {
                            fs::remove_file(&dst).context(FsError::DeleteFile((&dst).into()))?;
                        }
                    }

                    fs::copy(&src, &dst).context(FsError::CreateFile((&dst).into()))?;
                }

                CopyAction::CopyLink { src, dst } => {
                    symlink(&src, &dst).context(FsError::CreateFile((&dst).into()))?;
                }
            }
        }

        Ok(())
    }

    pub fn log(&self) {
        for action in &self.actions {
            if let CopyAction::CopyFile { ref src, ref dst } = action {
                info!("sync {} -> {}", pathlight(&src), pathlight(&dst))
            }
        }
    }
}

impl FromIterator<CopyAction> for CopyModel {
    fn from_iter<I: IntoIterator<Item = CopyAction>>(iter: I) -> Self {
        CopyModel::new(iter.into_iter().collect())
    }
}

/// Queries the filesystem and gets the date of the last time the file was modified keeped
/// by the system. Since this is a measurement made by the system, the time returned by this
/// function can be wrong in some cases: the user changed the date in it's system, an operation
/// was queued and performed at a later time and some other cases.
fn modified<P: AsRef<Path>>(file: P) -> Option<SystemTime> {
    if let Ok(data) = file.as_ref().metadata() {
        if let Ok(time) = data.modified() {
            return Some(time);
        }
    }

    warn!("Unable to access metadata for {}", pathlight(file.as_ref()));
    None
}

#[cfg(test)]
mod tests {
    use super::{
        modified, DirTree, FileSystemType, LinkedPoint, OverwriteMode, SyncActions, SyncModel,
        SyncOptions,
    };

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

    mod linked_point {
        use super::{modified, LinkedPoint, OverwriteMode};
        use std::{fs::File, thread, time};
        use tempfile;

        #[test]
        fn test_linked() {
            let dir = tmpdir!();
            let srcpath = create_file!(tmppath!(dir, "a.txt"));
            let dstpath = create_file!(tmppath!(dir, "b.txt"));
            let link = LinkedPoint::new(srcpath, dstpath);
            assert!(link.synced());
        }

        #[test]
        fn test_mirror_disallowed() {
            let dir = tmpdir!();
            let srcpath = create_file!(tmppath!(dir, "a.txt"));
            let dstpath = create_file!(tmppath!(dir, "b.txt"));
            let link = LinkedPoint::new(srcpath, dstpath);
            assert!(link.mirror(OverwriteMode::Disallow, false).is_err());
        }

        #[test]
        fn test_mirror_allow_copy() {
            let dir = tmpdir!();

            let dstpath = create_file!(tmppath!(dir, "b.txt"));
            thread::sleep(time::Duration::from_millis(2000));
            let srcpath = create_file!(tmppath!(dir, "a.txt"), "Hello, world");
            assert!(modified(&srcpath).unwrap() > modified(&dstpath).unwrap());

            let link = LinkedPoint::new(srcpath.clone(), dstpath.clone());
            assert!(link.mirror(OverwriteMode::Allow, false).is_ok());

            assert_eq!(read_file!(&dstpath), "Hello, world");
        }

        #[test]
        fn test_mirror_allow_not_copy() {
            let dir = tmpdir!();

            let srcpath = create_file!(tmppath!(dir, "a.txt"), "Hello, world");
            thread::sleep(time::Duration::from_millis(2000));
            let dstpath = create_file!(tmppath!(dir, "b.txt"));
            assert!(modified(&dstpath).unwrap() > modified(&srcpath).unwrap());

            let link = LinkedPoint::new(srcpath.clone(), dstpath.clone());
            assert!(link.mirror(OverwriteMode::Allow, false).is_ok());

            assert_ne!(read_file!(&dstpath), "Hello, world");
        }

        #[test]
        fn test_mirror_force() {
            let dir = tmpdir!();

            let srcpath = create_file!(tmppath!(dir, "a.txt"), "Hello, world");
            thread::sleep(time::Duration::from_millis(2000));
            let dstpath = create_file!(tmppath!(dir, "b.txt"));
            assert!(modified(&dstpath).unwrap() > modified(&srcpath).unwrap());

            let link = LinkedPoint::new(srcpath.clone(), dstpath.clone());
            assert!(link.mirror(OverwriteMode::Force, false).is_ok());

            assert_eq!(read_file!(&dstpath), "Hello, world");
        }

        #[test]
        fn test_mirror_force_allow() {
            let dir = tmpdir!();

            let dstpath = create_file!(tmppath!(dir, "b.txt"));
            thread::sleep(time::Duration::from_millis(2000));
            let srcpath = create_file!(tmppath!(dir, "a.txt"), "Hello, world");
            assert!(modified(&srcpath).unwrap() > modified(&dstpath).unwrap());

            let link = LinkedPoint::new(srcpath.clone(), dstpath.clone());
            assert!(link.mirror(OverwriteMode::Force, false).is_ok());

            assert_eq!(read_file!(&dstpath), "Hello, world");
        }
    }

    mod sync_model {
        use super::{DirTree, OverwriteMode, SyncActions, SyncModel, SyncOptions};
        use std::fs::{self, File};
        use tempfile;

        #[test]
        fn test_create_dir() {
            let (src, dst) = (tmpdir!(), tmpdir!());
            let tree = DirTree::new(src.path().into(), dst.path().into());
            let actions = vec![SyncActions::CreateDir("a".into())];
            let options = SyncOptions::new(false, false, OverwriteMode::Force);

            SyncModel::new(tree, actions, options).execute().unwrap();
            assert!(tmppath!(dst, "a").exists());
        }

        #[test]
        fn test_create_nested_dir() {
            let (src, dst) = (tmpdir!(), tmpdir!());
            let tree = DirTree::new(src.path().into(), dst.path().into());
            let actions = vec![SyncActions::CreateDir("a/b".into())];
            let options = SyncOptions::new(false, false, OverwriteMode::Force);

            SyncModel::new(tree, actions, options).execute().unwrap();
            assert!(tmppath!(dst, "a/b").exists());
        }

        #[test]
        fn test_create_file() {
            let (src, dst) = (tmpdir!(), tmpdir!());
            create_file!(tmppath!(src, "a.txt"), "aaaa");
            create_file!(tmppath!(src, "b.txt"), "bbbb");
            let tree = DirTree::new(src.path().into(), dst.path().into());

            let actions = vec![
                SyncActions::LinkFile("a.txt".into()),
                SyncActions::LinkFile("b.txt".into()),
            ];
            let options = SyncOptions::new(false, false, OverwriteMode::Force);

            SyncModel::new(tree, actions, options).execute().unwrap();
            assert!(tmppath!(dst, "a.txt").exists());
            assert!(tmppath!(dst, "b.txt").exists());
            assert_eq!(read_file!(tmppath!(dst, "a.txt")), "aaaa");
            assert_eq!(read_file!(tmppath!(dst, "b.txt")), "bbbb");
        }

        #[test]
        fn test_create_file_symbolic() {
            let (src, dst) = (tmpdir!(), tmpdir!());
            create_file!(tmppath!(src, "a.txt"), "aaaa");
            create_file!(tmppath!(src, "b.txt"), "bbbb");
            let tree = DirTree::new(src.path().into(), dst.path().into());

            let actions = vec![
                SyncActions::LinkFile("a.txt".into()),
                SyncActions::LinkFile("b.txt".into()),
            ];
            let mut options = SyncOptions::new(false, false, OverwriteMode::Force);
            options.symbolic = true;

            SyncModel::new(tree, actions, options).execute().unwrap();
            assert!(tmppath!(dst, "a.txt").exists());
            assert!(tmppath!(dst, "b.txt").exists());
            assert_eq!(read_file!(tmppath!(dst, "a.txt")), "aaaa");
            assert_eq!(read_file!(tmppath!(dst, "b.txt")), "bbbb");
            assert!(symlink!(tmppath!(dst, "a.txt")));
            assert!(symlink!(tmppath!(dst, "a.txt")));
        }

        #[test]
        fn test_sync_model_mixed() {
            let (src, dst) = (tmpdir!(), tmpdir!());
            fs::create_dir(tmppath!(src, "a")).expect("Unable to create folder");
            create_file!(tmppath!(src, "a/b.txt"), "bbbb");
            let tree = DirTree::new(src.path().into(), dst.path().into());

            let actions = vec![
                SyncActions::CreateDir("a".into()),
                SyncActions::LinkFile("a/b.txt".into()),
            ];
            let options = SyncOptions::new(false, false, OverwriteMode::Force);

            SyncModel::new(tree, actions, options).execute().unwrap();
            assert!(tmppath!(dst, "a").exists());
            assert!(tmppath!(dst, "a/b.txt").exists());
            assert_eq!(read_file!(tmppath!(dst, "a/b.txt")), "bbbb");
        }

        #[test]
        fn test_cleanup_model() {
            let (src, dst) = (tmpdir!(), tmpdir!());
            create_file!(tmppath!(dst, "c.txt"), "cccc");
            let tree = DirTree::new(src.path().into(), dst.path().into());

            let actions = vec![SyncActions::DeleteDst("c.txt".into())];
            let options = SyncOptions::new(false, false, OverwriteMode::Force);

            SyncModel::new(tree, actions, options).execute().unwrap();
            assert!(!tmppath!(dst, "c.txt").exists());
        }
    }

    mod dir_tree {
        use super::{DirTree, OverwriteMode, SyncActions, SyncOptions};
        use std::fs::{self, File};
        use tempfile;

        #[test]
        fn test_sync_copy_single_dir_model() {
            let options = SyncOptions::new(false, false, OverwriteMode::Force);

            let (src, dst) = (tmpdir!(), tmpdir!());
            create_file!(tmppath!(src, "a.txt"));
            create_file!(tmppath!(src, "b.txt"));

            let model = DirTree::new(src.path().into(), dst.path().into())
                .sync(options)
                .unwrap();

            assert_eq!(
                model.actions,
                vec![
                    SyncActions::LinkFile("a.txt".into()),
                    SyncActions::LinkFile("b.txt".into())
                ]
            );
        }

        #[test]
        fn test_sync_copy_symbolic() {
            let mut options = SyncOptions::new(false, false, OverwriteMode::Force);
            options.symbolic = true;

            let (src, dst) = (tmpdir!(), tmpdir!());
            create_file!(tmppath!(src, "a.txt"), "aaaa");
            create_file!(tmppath!(src, "b.txt"), "bbbb");

            let model = DirTree::new(src.path().into(), dst.path().into())
                .sync(options)
                .unwrap();

            assert_eq!(
                model.actions,
                vec![
                    SyncActions::LinkFile("a.txt".into()),
                    SyncActions::LinkFile("b.txt".into())
                ]
            );

            assert!(model.symbolic);
        }

        #[test]
        fn test_sync_copy_recursive() {
            let options = SyncOptions::new(false, false, OverwriteMode::Force);

            let (src, dst) = (tmpdir!(), tmpdir!());
            fs::create_dir(tmppath!(src, "c")).expect("Unable to create folder");
            create_file!(tmppath!(src, "c/d.txt"), "dddd");

            let model = DirTree::new(src.path().into(), dst.path().into())
                .sync(options)
                .unwrap();

            assert_eq!(
                model.actions,
                vec![
                    SyncActions::CreateDir("c".into()),
                    SyncActions::LinkFile("c/d.txt".into())
                ]
            );
        }

        #[test]
        fn test_sync_clean() {
            let options = SyncOptions::new(false, true, OverwriteMode::Force);

            let (src, dst) = (tmpdir!(), tmpdir!());
            create_file!(tmppath!(src, "a.txt"), "aaaa");
            create_file!(tmppath!(src, "b.txt"), "bbbb");
            create_file!(tmppath!(dst, "c.txt"), "cccc");

            let model = DirTree::new(src.path().into(), dst.path().into())
                .sync(options)
                .unwrap();

            assert_eq!(
                model.actions,
                vec![
                    SyncActions::LinkFile("a.txt".into()),
                    SyncActions::LinkFile("b.txt".into()),
                    SyncActions::DeleteDst("c.txt".into())
                ]
            )
        }
    }
}
