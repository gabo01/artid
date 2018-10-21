#![allow(dead_code)]

use failure::ResultExt;
use logger::pathlight;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::ffi::OsString;
use std::fs;
use std::iter::FromIterator;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use {AppError, FsError, Result};

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

/// Modifier options for the sync process in a DirTree. Check the properties to see which
/// behaviour they control.
#[derive(Debug, Copy, Clone)]
pub struct SyncOptions {
    /// Enables/Disables cleanup of files. If a file is present on the location to be written
    /// on but does not exist in it's supposed original location, the file will be deleted from
    /// the backup. This avoids generating garbage files on a backup dir.
    pub clean: bool,
    /// Controls how to handle if a location to be written on already exists. See OverwriteMode
    /// docs for more info on how this setting behaves.
    pub overwrite: OverwriteMode,
    /// Enables/Disables sync through symbolic links. If set to true a symbolic link will be
    /// created in the destination instead of copying the whole file.
    pub symbolic: bool,
}

impl SyncOptions {
    /// Creates a new set of options for the sync process.
    pub fn new(clean: bool, overwrite: OverwriteMode) -> Self {
        Self {
            clean,
            overwrite,
            symbolic: false,
        }
    }
}

/// Sets the mode for handling the case in which a file would be overwritten by the sync
/// operation.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum OverwriteMode {
    /// The function will raise an error if the location where it tries to write already
    /// exists.
    Disallow,
    /// The function will compare both locations last modification date. If the location
    /// to be written on is older than the location whose contents will be copied the
    /// location will be overwritten.
    Allow,
    /// The fuction will always overwrite the destination location regardless of the last
    /// modification date or any other parameter.
    Force,
}

#[derive(Debug)]
pub struct NewDirTree<'a> {
    src: &'a Path,
    dst: &'a Path,
    root: TreeNode,
}

impl<'a> NewDirTree<'a> {
    pub fn new(src: &'a Path, dst: &'a Path) -> Result<Self> {
        let (srcexists, dstexists) = (src.exists(), dst.exists());
        let presence = if srcexists && dstexists {
            Presence::Both
        } else if srcexists {
            Presence::Src
        } else {
            Presence::Dst
        };

        let mut root = TreeNode::new("".into(), presence, FileSystemType::Dir);
        root.read_recursive(&src, &dst)?;

        Ok(Self { src, dst, root })
    }

    pub fn iter<'b>(&'b self) -> TreeIter<'a, 'b> {
        TreeIter::new(self)
    }
}

impl<'a, 'b> IntoIterator for &'b NewDirTree<'a>
where
    'a: 'b,
{
    type Item = IterTreeNode<'a, 'b>;
    type IntoIter = TreeIter<'a, 'b>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Debug)]
pub struct TreeNode {
    path: PathBuf,
    presence: Presence,
    kind: FileSystemType,
    children: Option<Vec<TreeNode>>,
}

impl TreeNode {
    pub fn new(path: PathBuf, presence: Presence, kind: FileSystemType) -> Self {
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
                if child.kind == FileSystemType::Dir {
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
                            TreeNode::new(
                                self.path.join(name),
                                Presence::Src,
                                FileSystemType::from(path),
                            )
                        }).collect(),
                );
            }

            Presence::Dst => {
                self.children = Some(
                    read!(&dst)
                        .map(|(path, name)| {
                            TreeNode::new(
                                self.path.join(name),
                                Presence::Dst,
                                FileSystemType::from(path),
                            )
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
            .map(|(key, value)| {
                TreeNode::new(path.join(key), value.1, FileSystemType::from(value.0))
            }).collect();

        Ok(vec)
    }
}

pub struct TreeIter<'a, 'b>
where
    'a: 'b,
{
    tree: &'b NewDirTree<'a>,
    elements: VecDeque<IterTreeNode<'a, 'b>>,
}

impl<'a, 'b> TreeIter<'a, 'b>
where
    'a: 'b,
{
    pub fn new(tree: &'b NewDirTree<'a>) -> Self {
        let mut elements = VecDeque::new();
        elements.push_back(IterTreeNode::new(&tree.src, &tree.dst, &tree.root));

        Self { tree, elements }
    }
}

impl<'a, 'b> Iterator for TreeIter<'a, 'b>
where
    'a: 'b,
{
    type Item = IterTreeNode<'a, 'b>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.elements.pop_front();

        match next {
            Some(val) => {
                if val.node.kind == FileSystemType::Dir {
                    if let Some(ref children) = val.node.children {
                        self.elements.extend(
                            children
                                .iter()
                                .map(|e| IterTreeNode::new(val.src, val.dst, e)),
                        );
                    }
                }

                Some(val)
            }

            None => None,
        }
    }
}

pub struct IterTreeNode<'a, 'b> {
    pub src: &'a Path,
    pub dst: &'a Path,
    pub node: &'b TreeNode,
}

impl<'a, 'b> IterTreeNode<'a, 'b> {
    pub fn new(src: &'a Path, dst: &'a Path, node: &'b TreeNode) -> Self {
        Self { src, dst, node }
    }

    pub fn presence(&self) -> Presence {
        self.node.presence
    }

    pub fn kind(&self) -> FileSystemType {
        self.node.kind
    }

    pub fn path(&self) -> &Path {
        &self.node.path
    }

    pub fn synced(&self, direction: Direction) -> bool {
        match direction {
            Direction::Forward => LinkedPoint::new(
                self.src.join(&self.node.path),
                self.dst.join(&self.node.path),
            ).synced(),

            Direction::Backward => LinkedPoint::new(
                self.dst.join(&self.node.path),
                self.src.join(&self.node.path),
            ).synced(),
        }
    }
}

pub enum Direction {
    Forward,
    Backward,
}

/// Represents two different linked directory trees. The dst path is seen as the 'link'
/// and the src path is seen as the 'linked place'. This means that syncing the link is making
/// a copy of all files in src to dst.
///
/// The idea behind this type is to be able to walk the dest path and mimic it's structure on
/// the origin path.
///
/// Creation of this type won't fail even if the given path's aren't valid. You can check if the
/// given path's are correct by calling .valid(). If the path's are not correct the sync function
/// will fail and return an appropiate error.
#[derive(Debug)]
pub struct DirTree {
    src: PathBuf,
    dst: PathBuf,
}

impl DirTree {
    /// Creates a new link representation for two different trees.
    pub fn new(src: PathBuf, dst: PathBuf) -> Self {
        Self { src, dst }
    }

    ///
    #[allow(dead_code)]
    pub fn compare(&self) -> Result<Vec<Element>> {
        let mut vec = vec![];
        let src = Self::walk(&self.src)?;
        let dst = Self::walk(&self.dst)?
            .into_iter()
            .filter(|x| !src.iter().any(|y| y.path == x.path))
            .collect::<Vec<Entry<'_>>>();

        for entry in &src {
            if self.dst.join(&entry.path).exists() {
                let link = {
                    let forward = LinkedPoint::new(
                        self.src.join(entry.path.clone()),
                        self.dst.join(entry.path.clone()),
                    ).synced();

                    let backward = LinkedPoint::new(
                        self.dst.join(entry.path.clone()),
                        self.src.join(entry.path.clone()),
                    ).synced();

                    if forward {
                        Link::Forward
                    } else if backward {
                        Link::Backward
                    } else {
                        Link::None
                    }
                };

                vec.push(Element {
                    path: entry.path.clone(),
                    link,
                    presence: Presence::Both,
                })
            } else {
                vec.push(Element {
                    path: entry.path.clone(),
                    link: Link::None,
                    presence: Presence::Src,
                });
            }
        }

        for entry in &dst {
            vec.push(Element {
                path: entry.path.clone(),
                link: Link::None,
                presence: Presence::Dst,
            });
        }

        Ok(vec)
    }

    /// Syncs the two trees. This function will fail if the two points aren't linked
    /// and it is unable to create the dst dir, the 'link' or if it is unable to
    /// read the contents of the src, the 'linked', dir.
    ///
    /// Behaviour of these function can be controlled through the options sent for things such
    /// as file clashes, errors while processing a file or a subdirectory and other things. See
    /// SyncOptions docs for more info on these topic.
    pub fn sync(self, options: SyncOptions) -> Result<(SyncModel)> {
        let mut actions = vec![];

        for entry in Self::walk(&self.src)? {
            match entry.kind() {
                FileSystemType::Dir => {
                    if !self.dst.join(&entry.path).exists() {
                        actions.push(SyncActions::CreateDir(entry.path))
                    }
                }

                FileSystemType::File => actions.push(SyncActions::LinkFile(entry.path)),

                FileSystemType::Other => {
                    warn!("Unable to process {}", pathlight(entry.full_path()))
                }
            }
        }

        if options.clean && self.dst.exists() {
            for entry in Self::walk(&self.dst)? {
                match entry.kind() {
                    FileSystemType::Dir | FileSystemType::File => {
                        if !self.src.join(&entry.path).exists() {
                            actions.push(SyncActions::DeleteDst(entry.path))
                        }
                    }

                    FileSystemType::Other => {
                        warn!("Unable to process {}", pathlight(entry.full_path()))
                    }
                }
            }
        }

        Ok(SyncModel::new(self, actions, options))
    }

    fn walk(dir: &Path) -> Result<Vec<Entry<'_>>> {
        let mut entries = vec![Entry::new(dir, "".into(), 0)];
        let mut walked = Self::walk_recursive(&entries[0])?;
        entries.append(&mut walked);
        Ok(entries)
    }

    fn walk_recursive<'a>(entry: &Entry<'a>) -> Result<Vec<Entry<'a>>> {
        let mut entries = vec![];

        for element in fs::read_dir(entry.full_path())
            .context(FsError::ReadFile(entry.full_path().into()))?
            .into_iter()
            .filter_map(|e| e.ok())
        {
            entries.push(Entry::new(
                entry.root,
                entry.path.join(element.file_name()),
                entry.deepness + 1,
            ));

            if let FileSystemType::Dir = FileSystemType::from(element.path()) {
                let mut walked = Self::walk_recursive(&entries.last().unwrap())?;
                entries.append(&mut walked);
            }
        }

        Ok(entries)
    }
}

struct Entry<'a> {
    root: &'a Path,
    path: PathBuf,
    deepness: u8,
}

impl<'a> Entry<'a> {
    pub fn new(root: &'a Path, path: PathBuf, deepness: u8) -> Self {
        Self {
            root,
            path,
            deepness,
        }
    }

    pub fn kind(&self) -> FileSystemType {
        FileSystemType::from(self.root.join(&self.path))
    }

    pub fn full_path(&self) -> PathBuf {
        self.root.join(&self.path)
    }
}

///
#[derive(Debug)]
pub struct Element {
    pub path: PathBuf,
    pub link: Link,
    pub presence: Presence,
}

///
#[derive(Debug, Eq, PartialEq)]
pub enum Link {
    Forward,
    Backward,
    None,
}

///
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Presence {
    Src,
    Dst,
    Both,
}

#[derive(Copy, Clone, Debug, PartialEq)]
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

pub enum TreeModelActions {
    CreateDir { src: PathBuf, dst: PathBuf },
    CopyFile { src: PathBuf, dst: PathBuf },
    CopyLink { src: PathBuf, dst: PathBuf },
}

pub struct TreeModel {
    actions: Vec<TreeModelActions>,
}

impl TreeModel {
    pub fn new(actions: Vec<TreeModelActions>) -> Self {
        Self { actions }
    }

    pub fn execute(self) -> Result<()> {
        for action in self.actions {
            match action {
                TreeModelActions::CreateDir { src, dst } => {
                    LinkedPoint::new(src, dst).create_dir()?;
                }

                TreeModelActions::CopyFile { src, dst } => {
                    LinkedPoint::new(src, dst).copy()?;
                }

                TreeModelActions::CopyLink { src, dst } => {
                    LinkedPoint::new(src, dst).link()?;
                }
            }
        }

        Ok(())
    }

    pub fn log(&self) {
        for action in &self.actions {
            if let TreeModelActions::CopyFile { ref src, ref dst } = action {
                info!("sync {} -> {}", pathlight(&src), pathlight(&dst))
            }
        }
    }
}

impl FromIterator<ModelItem> for TreeModel {
    fn from_iter<I: IntoIterator<Item = ModelItem>>(iter: I) -> Self {
        TreeModel::new(
            iter.into_iter()
                .map(|e| match e.method {
                    Method::Dir => TreeModelActions::CreateDir {
                        src: e.src,
                        dst: e.dst,
                    },

                    Method::Copy => TreeModelActions::CopyFile {
                        src: e.src,
                        dst: e.dst,
                    },

                    Method::Link => TreeModelActions::CopyLink {
                        src: e.src,
                        dst: e.dst,
                    },
                }).collect(),
        )
    }
}

#[derive(Eq, PartialEq)]
pub enum Method {
    Copy,
    Link,
    Dir,
}

pub struct ModelItem {
    src: PathBuf,
    dst: PathBuf,
    method: Method,
}

impl ModelItem {
    pub fn new(src: PathBuf, dst: PathBuf, method: Method) -> Self {
        Self { src, dst, method }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum SyncActions {
    CreateDir(PathBuf),
    LinkFile(PathBuf),
    DeleteDst(PathBuf),
}

#[derive(Debug)]
pub struct SyncModel {
    src: PathBuf,
    dst: PathBuf,
    actions: Vec<SyncActions>,
    overwrite: OverwriteMode,
    symbolic: bool,
}

impl SyncModel {
    pub fn new(tree: DirTree, actions: Vec<SyncActions>, options: SyncOptions) -> Self {
        Self {
            src: tree.src,
            dst: tree.dst,
            actions,
            overwrite: options.overwrite,
            symbolic: options.symbolic,
        }
    }

    pub fn execute(self) -> Result<()> {
        for action in self.actions {
            match action {
                SyncActions::CreateDir(dir) => fs::create_dir_all(self.dst.join(dir))
                    .context(FsError::OpenFile((&self.dst).into()))?,

                SyncActions::LinkFile(ref link) => {
                    LinkedPoint::new(self.src.join(link), self.dst.join(link))
                        .mirror(self.overwrite, self.symbolic)?;
                }

                SyncActions::DeleteDst(ref path) => {
                    let full_path = self.dst.join(path);

                    match FileSystemType::from(&full_path) {
                        FileSystemType::Dir => {
                            fs::remove_dir_all(&full_path)
                                .context(FsError::DeleteFile(full_path.into()))?;
                        }

                        FileSystemType::File => {
                            fs::remove_file(&full_path)
                                .context(FsError::DeleteFile(full_path.into()))?;
                        }

                        FileSystemType::Other => {
                            warn!("Unable to identify {}", pathlight(full_path))
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn log(&self) {
        for action in &self.actions {
            if let SyncActions::LinkFile(ref dir) = action {
                info!(
                    "Sync {} -> {}",
                    pathlight(self.src.join(dir)),
                    pathlight(self.dst.join(dir))
                );
            }
        }
    }
}

/// Represents a link between two different paths points. The dst path is seen as the
/// 'link's location while the src path is seen as the link's pointed place.
#[derive(Debug)]
struct LinkedPoint {
    src: PathBuf,
    dst: PathBuf,
}

impl LinkedPoint {
    /// Creates a link representation of two different locations.
    pub(self) fn new(src: PathBuf, dst: PathBuf) -> Self {
        Self { src, dst }
    }

    /// Checks if the two points are already linked in the filesystem. Two points are linked
    /// if they both exist and the modification date of origin is equal or newer than dest.
    pub(self) fn synced(&self) -> bool {
        if self.src.exists() && self.dst.exists() {
            if let Some(linked) = modified(&self.src) {
                if let Some(link) = modified(&self.dst) {
                    return link >= linked;
                }
            }
        }

        false
    }

    pub fn create_dir(&self) -> Result<()> {
        if !self.dst.exists() {
            fs::create_dir(&self.dst).context(FsError::OpenFile((&self.dst).into()))?;
        }

        Ok(())
    }

    pub fn copy(&self) -> Result<()> {
        if let Ok(metadata) = fs::symlink_metadata(&self.dst) {
            if metadata.file_type().is_symlink() {
                fs::remove_file(&self.dst).context(FsError::DeleteFile((&self.dst).into()))?;
            }
        }

        fs::copy(&self.src, &self.dst).context(FsError::CreateFile((&self.dst).into()))?;
        Ok(())
    }

    pub fn link(&self) -> Result<()> {
        Self::symlink(&self.src, &self.dst).context(FsError::CreateFile((&self.dst).into()))?;
        Ok(())
    }

    /// Syncs (or Links) the two points on the filesystem. The behaviour of this function
    /// for making the sync is controlled by the overwrite option. See the docs for
    /// OverwriteMode to get more info.
    ///
    /// The behaviour is also controlled by the symbolic parameter. If set to true the
    /// function will create a symbolic link instead of copying the file.
    pub(self) fn mirror(&self, overwrite: OverwriteMode, symbolic: bool) -> Result<()> {
        if overwrite == OverwriteMode::Disallow && self.dst.exists() {
            err!(FsError::PathExists((&self.dst).into()));
        }

        if overwrite == OverwriteMode::Allow && self.synced() {
            return Ok(());
        }

        if !symbolic {
            if let Ok(metadata) = fs::symlink_metadata(&self.dst) {
                if metadata.file_type().is_symlink() {
                    fs::remove_file(&self.dst).context(FsError::DeleteFile((&self.dst).into()))?;
                }
            }

            fs::copy(&self.src, &self.dst).context(FsError::CreateFile((&self.dst).into()))?;
        } else {
            Self::symlink(&self.src, &self.dst).context(FsError::CreateFile((&self.dst).into()))?;
        }

        info!(
            "synced: {} -> {}",
            pathlight(&self.src),
            pathlight(&self.dst)
        );

        Ok(())
    }

    /// Intended to create a symlink on Unix operating systems
    #[cfg(unix)]
    fn symlink<P: AsRef<Path>, T: AsRef<Path>>(src: P, dst: T) -> ::std::io::Result<()> {
        use std::os::unix::fs::symlink;
        symlink(src, dst)
    }

    /// Intended to create a symlink on Windows operating systems
    #[cfg(windows)]
    fn symlink<P: AsRef<Path>, T: AsRef<Path>>(src: P, dst: T) -> ::std::io::Result<()> {
        use std::os::windows::fs::symlink_file as symlink;
        symlink(src, dst)
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
