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

/// Reduce the boilerplate when reading a directory
macro_rules! read {
    ($path:expr) => {
        fs::read_dir($path)
            .context(FsError::ReadFile($path.into()))?
            .into_iter()
            .filter_map(|e| e.ok())
            .map(|e| (e.path(), e.file_name()))
    };
}

/// A DirTree is a structure that makes a graph about the contents of two directories. The idea is to
/// be able to take a relative path to the root of the dir and know if it is present in one or
/// both directories and other important metadata.
///
/// Trying to create a DirTree may result in an error because, in order to create it, the filesystem
/// must be queried.
#[derive(Debug)]
pub struct DirTree<'a> {
    src: &'a Path,
    dst: &'a Path,
    root: TreeNode,
}

impl<'a> DirTree<'a> {
    /// Creates a new comparison DirTree from two path references. As told in the type level
    /// documentation, creation of this tree may fail.
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

    /// Creates an iterator over the elements of the tree. See TreeIter for details about the
    /// iterator created by this method
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

/// Given a path node from the tree, this value represents the path's presence in the
/// source directory, destination directory or both of them. The None option is not covered since
/// a non present path would not be in the tree in first place.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Presence {
    Src,
    Dst,
    Both,
}

/// Given a path node from the tree, this value represents the kind of element the node represents
/// on the filesystem. A symlink will represent the element is points to.
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

/// The building blocks of the DirTree (a.k.a nodes). Each node represents a relative path from
/// the root of the tree and carries the needed metadata to be operated over. Nodes are identified
/// by their full relative path to the tree root. This is done because each node contains it's
/// children but does not have a reference to it's parent.
///
/// Currently, the important metadata stored on the node at the moment of it's creation consists of
/// the presence data (determines if the node is present on the source, destination or both trees)
/// and the kind of node it represents. The kind of node it represents is subject to change since,
/// right now, it is undefined behaviour what happens if a path is a kind of node in the source and
/// another kind on the destination.
#[derive(Debug)]
pub struct TreeNode {
    path: PathBuf,
    presence: Presence,
    kind: FileType,
    children: Option<Vec<TreeNode>>,
}

impl TreeNode {
    /// Creates a new tree node to be inserted on a tree. In order to insert the node into a tree
    /// it must be specified as the children of another node or as the root.
    ///
    /// When a node is created, since there is no associated tree, the presence and kind values must
    /// be supplied based on the tree where it is going to be inserted and the children are defaulted
    /// to none.
    pub fn new(path: PathBuf, presence: Presence, kind: FileType) -> Self {
        Self {
            path,
            presence,
            kind,
            children: None,
        }
    }

    /// Performs the read in search of the children of the node (see read function docs). The only
    /// difference is that these function will be also applied to the children found in order to
    /// build a full tree with this node as the root.
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

    /// Get the children of this node of the tree, a Dir kind of node since for a file
    /// it makes no sense. To get the children, a source and destination, these are the source
    /// and destination paths of the tree, must be provided. Every element found during
    /// the read process will be added to as children of this node.
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

    /// Used to sort the elements found in both locations of the tree, source and destination.
    /// This is, esentially, take two lists and make a third list where each element knows if it
    /// belonged to the first, second or both lists.
    ///
    /// Implementation details:
    ///     This function works using a hash table to properly map the elements to the third
    ///     list. This means that the resulting third list is not guaranteed to be ordered in
    ///     the same way the elements were yielded by the file system.
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

/// Iterator over the nodes of the directory tree. This iterator does not yield the nodes
/// of the tree directly. Instead, it yields a wrapper type called TreeIterNode that contains
/// a reference to the node and the paths of the tree. This way, the yielded element is complete
/// and can be queried for aditional information aside from the contained in the pude node.
///
/// This iterator goes through the tree nodes by levels, it goes through each level before going
/// to the next one. In other words, is an horizontal iterator.
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
    /// Creates the horizontal iterator based on the given tree
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

/// Represents a syncing direction. Given a node, the path it references can be translated into
/// two file system locations. Ensuring the locations are synced (in a direction) means taking
/// the modified time in one location and checking if it is lower than the modified time in the
/// other direction.
///
/// Even if this approach is naive in a general case, since one of the location is a backup
/// location this approach works. The other option would be making a full file comparison, but that
/// would tax heavily on performance if the files are big.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Direction {
    /// The forward direction means ensuring that the source tree location modified time is lower
    /// than the destination tree location's modified time.
    Forward,
    /// The backward direction means ensuring that the destination tree location modified time is
    /// lower than the source tree location's modified time.
    Backward,
}

/// Represents the elements yielded by the TreeIter iterator. As told in the documentation of the
/// TreeIter type, this element takes the pure node and combines it with the tree's metadata to
/// have a full link between two filesystem locations.
#[derive(Debug)]
pub struct TreeIterNode<'a, 'b> {
    pub src: &'a Path,
    pub dst: &'a Path,
    pub node: &'b TreeNode,
}

impl<'a, 'b> TreeIterNode<'a, 'b> {
    /// Creates the node from the pieces of information needed. Src and dst are the paths stored
    /// on the tree.
    pub fn new(src: &'a Path, dst: &'a Path, node: &'b TreeNode) -> Self {
        Self { src, dst, node }
    }

    /// Returns the presence attribute of the node. For more information about what the presence
    /// attribute is see the docs for Presence and TreeNode.
    pub fn presence(&self) -> Presence {
        self.node.presence
    }

    /// Returns the kind attribute of the node. For more information about what the kind
    /// attribute is see the docs for FileType and TreeNode.
    pub fn kind(&self) -> FileType {
        self.node.kind
    }

    /// Returns the relative path to the root that is stored inside the pure node.
    pub fn path(&self) -> &Path {
        &self.node.path
    }

    /// Checks if the locations referenced by this node are synced given a syncing direction.
    /// See the docs of Direction to know more about what this check represents.
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

/// Represents the actions to perform in order to execute a full copy model for a specific operation.
/// These operations can be, but are not limited to, backup and restore. The different variants
/// represent the different operations an action can take. It is expected that these actions are
/// determined based on a comparison tree of the two locations which should have been previously
/// built.
///
/// Based on a list of these actions, a copy model can be built to describe the whole process before
/// making it.
pub enum CopyAction {
    /// Creates a directory on the target location, this is expected to be done if a tree node
    /// is present in one location but not in the other.
    CreateDir { target: PathBuf },
    /// Performs a full copy of the file from src to dst, a thing to notice is that in complex
    /// operations, src and dst may not exactly match the result of taking src and dst + the node
    /// path from the tree.
    CopyFile { src: PathBuf, dst: PathBuf },
    /// Creates a symlink on dst that points to src. As said on the CopyFile docs, src and dst may
    /// not exactly match, but are supposed to be derived from, the src and dst of the comparison
    /// tree.
    CopyLink { src: PathBuf, dst: PathBuf },
}

/// Copy model for a specific operation. Take an operation such as a backup, you can describe that
/// operation with a series of actions such as: create dir a, copy file b to c. These model is
/// esentially a list of actions to perform in order to say an operation has been done.
///
/// As such, since the model is a list of actions, it can and is constructed from a list of actions
/// to perform
pub struct CopyModel {
    actions: Vec<CopyAction>,
}

impl CopyModel {
    /// Constructs a new CopyModel from a list of actions. It is also a valid target for collection
    /// of an iterator of CopyAction
    pub fn new(actions: Vec<CopyAction>) -> Self {
        Self { actions }
    }

    /// Executes the model. This means going through each action and performing the related procedure.
    /// The procedures for each possible action are documented in the docs of CopyAction.
    ///
    /// An important thing to notice is that this functions returns an error, it may imply the
    /// model was partially executed and that a cleanup operation of the partial execution is needed.
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

    /// Logs the model actions, it is perfect to perform a dry run of an operation without actually
    /// doing it. For simplicity, only the files to be copied are reported since the other model
    /// actions may be seen as boilerplate to copy the files.
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
    use super::{CopyAction, CopyModel, Direction, FileType, Presence, TreeIterNode, TreeNode};
    use std::fs::File;
    use tempfile;

    #[test]
    fn test_linked() {
        let (src, dst) = (tmpdir!(), tmpdir!());
        create_file!(tmppath!(src, "a.txt"));
        create_file!(tmppath!(dst, "a.txt"));
        let node = TreeNode::new("".into(), Presence::Both, FileType::File);

        assert!(TreeIterNode::new(src.path(), dst.path(), &node).synced(Direction::Forward));
        assert!(TreeIterNode::new(dst.path(), src.path(), &node).synced(Direction::Backward))
    }

    mod file_system {
        use super::FileType;
        use std::fs::File;
        use tempfile;

        #[test]
        fn test_system_dir() {
            let dir = tmpdir!();
            assert_eq!(FileType::from(dir.path()), FileType::Dir);
        }

        #[test]
        fn test_system_file() {
            let dir = tmpdir!();
            let path = create_file!(tmppath!(dir, "a.txt"));
            assert_eq!(FileType::from(path), FileType::File);
        }
    }

    mod copy_model {
        use super::{CopyAction, CopyModel};
        use std::fs::File;
        use tempfile;

        #[test]
        fn test_create_dir() {
            let dir = tmpdir!();
            let action = CopyAction::CreateDir {
                target: dir.path().join("asd"),
            };

            let model = vec![action].into_iter().collect::<CopyModel>();
            model.execute().expect("Unable to execute model");
            assert!(tmppath!(dir, "asd").exists());
            assert!(tmppath!(dir, "asd").is_dir());
        }

        #[test]
        fn test_create_file() {
            let (src, dst) = (tmpdir!(), tmpdir!());
            let a_path = create_file!(tmppath!(src, "a.txt"), "aaaa");
            let b_path = create_file!(tmppath!(src, "b.txt"), "bbbb");

            let actions = vec![
                CopyAction::CopyFile {
                    src: a_path.clone(),
                    dst: tmppath!(dst, "a.txt"),
                },
                CopyAction::CopyFile {
                    src: b_path.clone(),
                    dst: tmppath!(dst, "b.txt"),
                },
            ];

            let model = actions.into_iter().collect::<CopyModel>();
            model.execute().expect("Unable to execute model");

            assert!(tmppath!(dst, "a.txt").exists());
            assert!(tmppath!(dst, "b.txt").exists());
            assert_eq!(read_file!(tmppath!(dst, "a.txt")), "aaaa");
            assert_eq!(read_file!(tmppath!(dst, "b.txt")), "bbbb");
        }

        #[test]
        fn test_create_file_symbolic() {
            let (src, dst) = (tmpdir!(), tmpdir!());
            let a_path = create_file!(tmppath!(src, "a.txt"), "aaaa");
            let b_path = create_file!(tmppath!(src, "b.txt"), "bbbb");

            let actions = vec![
                CopyAction::CopyLink {
                    src: a_path.clone(),
                    dst: tmppath!(dst, "a.txt"),
                },
                CopyAction::CopyLink {
                    src: b_path.clone(),
                    dst: tmppath!(dst, "b.txt"),
                },
            ];

            let model = actions.into_iter().collect::<CopyModel>();
            model.execute().expect("Unable to execute model");

            assert!(tmppath!(dst, "a.txt").exists());
            assert!(tmppath!(dst, "b.txt").exists());
            assert_eq!(read_file!(tmppath!(dst, "a.txt")), "aaaa");
            assert_eq!(read_file!(tmppath!(dst, "b.txt")), "bbbb");
            assert!(symlink!(tmppath!(dst, "a.txt")));
            assert!(symlink!(tmppath!(dst, "a.txt")));
        }

        #[test]
        fn test_mixed_model() {
            let src = tmpdir!();
            let dst = tmppath!(src, "target");
            let a_path = create_file!(tmppath!(src, "a.txt"), "aaaa");
            let b_path = create_file!(tmppath!(src, "b.txt"), "bbbb");

            let actions = vec![
                CopyAction::CreateDir {
                    target: dst.clone(),
                },
                CopyAction::CopyFile {
                    src: a_path,
                    dst: dst.join("a.txt"),
                },
                CopyAction::CopyLink {
                    src: b_path,
                    dst: dst.join("b.txt"),
                },
            ];

            let model = actions.into_iter().collect::<CopyModel>();
            model.execute().expect("Unable to execute model");

            assert!(tmppath!(src, "target").exists());
            assert!(tmppath!(src, "target").is_dir());
            assert!(tmppath!(src, "target/a.txt").exists());
            assert_eq!(read_file!(tmppath!(src, "target/a.txt")), "aaaa");
            assert!(tmppath!(src, "target/b.txt").exists());
            assert!(symlink!(tmppath!(src, "target/b.txt")));
        }
    }
}
