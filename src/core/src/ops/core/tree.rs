use log::{log, warn};
use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Reduce the boilerplate when reading a directory
macro_rules! read {
    ($path:expr) => {
        fs::read_dir($path)?
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
    pub fn new(src: &'a Path, dst: &'a Path) -> ::std::io::Result<Self> {
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
    #[allow(missing_docs)]
    Src,
    #[allow(missing_docs)]
    Dst,
    #[allow(missing_docs)]
    Both,
}

/// Given a path node from the tree, this value represents the kind of element the node represents
/// on the filesystem. A symlink will represent the element is points to.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FileType {
    #[allow(missing_docs)]
    File,
    #[allow(missing_docs)]
    Dir,
    #[allow(missing_docs)]
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
#[derive(Debug, Eq, PartialEq)]
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
    pub fn read_recursive<T, P>(&mut self, src: T, dst: P) -> ::std::io::Result<()>
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
    pub fn read<T: AsRef<Path>, P: AsRef<Path>>(
        &mut self,
        src: T,
        dst: P,
    ) -> ::std::io::Result<()> {
        let src = src.as_ref().join(&self.path);
        let dst = dst.as_ref().join(&self.path);

        match self.presence {
            Presence::Both => {
                self.children = Some(Self::compare(&self.path, read!(&src), read!(&dst)));
            }

            Presence::Src => {
                self.children = Some(
                    read!(&src)
                        .map(|(path, name)| {
                            TreeNode::new(self.path.join(name), Presence::Src, FileType::from(path))
                        })
                        .collect(),
                );
            }

            Presence::Dst => {
                self.children = Some(
                    read!(&dst)
                        .map(|(path, name)| {
                            TreeNode::new(self.path.join(name), Presence::Dst, FileType::from(path))
                        })
                        .collect(),
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
    fn compare<P, T, U>(path: P, src: T, dst: U) -> Vec<TreeNode>
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

        table
            .into_iter()
            .map(|(key, value)| TreeNode::new(path.join(key), value.1, FileType::from(value.0)))
            .collect()
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
    #[allow(missing_docs)]
    pub src: &'a Path,
    #[allow(missing_docs)]
    pub dst: &'a Path,
    #[allow(missing_docs)]
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

    warn!("Unable to access metadata for {}", file.as_ref().display());
    None
}

#[cfg(test)]
mod tests {
    use super::{DirTree, Direction, FileType, Presence, TreeIterNode, TreeNode};
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

    mod tree {
        use super::{DirTree, FileType, Presence, TreeNode};
        use std::fs::{self, File};
        use std::path::PathBuf;
        use tempfile;

        fn sort(node: &mut TreeNode) {
            if let Some(ref mut children) = node.children {
                children.sort_unstable_by(|a, b| a.path.cmp(&b.path));

                for mut child in children {
                    sort(&mut child);
                }
            }
        }

        fn generate_tree() -> (tempfile::TempDir, tempfile::TempDir) {
            let (src, dst) = (tmpdir!(), tmpdir!());

            // source tree
            create_file!(tmppath!(src, "a.txt"));
            create_file!(tmppath!(src, "b.txt"));
            fs::create_dir_all(tmppath!(src, "bin")).expect("Unable to create dir");
            create_file!(tmppath!(src, "bin/c.txt"));
            create_file!(tmppath!(src, "bin/d.txt"));

            // destination tree
            create_file!(tmppath!(dst, "a.txt"));
            fs::create_dir_all(tmppath!(dst, "bin")).expect("Unable to create dir");
            create_file!(tmppath!(dst, "bin/c.txt"));
            create_file!(tmppath!(dst, "bin/d.txt"));
            create_file!(tmppath!(dst, "e.txt"));
            fs::create_dir_all(tmppath!(dst, "target")).expect("Unable to create dir");

            (src, dst)
        }

        #[test]
        fn test_dir_root() {
            let (src, dst) = (tmpdir!(), tmpdir!());
            let unexistant = src.path().join("unexistant");

            // test with an unexistant dst
            let tree = DirTree::new(src.path(), &unexistant).expect("Failed on tree creation");
            assert_eq!(tree.root.presence, Presence::Src);

            // test with an unexistant src
            let tree = DirTree::new(&unexistant, dst.path()).expect("Failed on tree creation");
            assert_eq!(tree.root.presence, Presence::Dst);

            // test with both points
            let tree = DirTree::new(src.path(), dst.path()).expect("Failed on tree creation");
            assert_eq!(tree.root.presence, Presence::Both);
        }

        #[test]
        fn test_node_read_empty() {
            let (src, dst) = (tmpdir!(), tmpdir!());
            let unexistant = dst.path().join("unexistant");

            let mut node = TreeNode::new("".into(), Presence::Src, FileType::Dir);
            node.read(src.path(), &unexistant)
                .expect("Unable to read the directory");

            assert!(node.children.is_some());
        }

        #[test]
        fn test_node_read() {
            let (src, dst) = (tmpdir!(), tmpdir!());
            let unexistant = dst.path().join("unexistant");

            create_file!(tmppath!(src, "a.txt"));
            create_file!(tmppath!(src, "b.txt"));
            fs::create_dir_all(tmppath!(src, "c")).expect("Unable to create dir");
            create_file!(tmppath!(src, "c/d.txt"));

            let mut node = TreeNode::new("".into(), Presence::Src, FileType::Dir);
            node.read(src.path(), &unexistant)
                .expect("Unable to read the directory");
            sort(&mut node);

            assert!(node.children.is_some());
            assert_eq!(
                node.children.unwrap(),
                vec![
                    TreeNode {
                        path: PathBuf::from("a.txt"),
                        presence: Presence::Src,
                        kind: FileType::File,
                        children: None
                    },
                    TreeNode {
                        path: PathBuf::from("b.txt"),
                        presence: Presence::Src,
                        kind: FileType::File,
                        children: None
                    },
                    TreeNode {
                        path: PathBuf::from("c"),
                        presence: Presence::Src,
                        kind: FileType::Dir,
                        children: None
                    }
                ]
            );
        }

        #[test]
        fn test_node_read_recursive() {
            let (src, dst) = (tmpdir!(), tmpdir!());
            let unexistant = dst.path().join("unexistant");

            create_file!(tmppath!(src, "a.txt"));
            create_file!(tmppath!(src, "b.txt"));
            fs::create_dir_all(tmppath!(src, "c")).expect("Unable to create dir");
            create_file!(tmppath!(src, "c/d.txt"));

            let mut node = TreeNode::new("".into(), Presence::Src, FileType::Dir);
            node.read_recursive(src.path(), &unexistant)
                .expect("Unable to read the directory");
            sort(&mut node);

            assert!(node.children.is_some());
            assert_eq!(
                node.children.unwrap(),
                vec![
                    TreeNode {
                        path: PathBuf::from("a.txt"),
                        presence: Presence::Src,
                        kind: FileType::File,
                        children: None
                    },
                    TreeNode {
                        path: PathBuf::from("b.txt"),
                        presence: Presence::Src,
                        kind: FileType::File,
                        children: None
                    },
                    TreeNode {
                        path: PathBuf::from("c"),
                        presence: Presence::Src,
                        kind: FileType::Dir,
                        children: Some(vec![TreeNode {
                            path: PathBuf::from("c/d.txt"),
                            presence: Presence::Src,
                            kind: FileType::File,
                            children: None
                        }])
                    }
                ]
            );
        }

        #[test]
        fn test_tree_generation() {
            let (src, dst) = generate_tree();

            let mut tree = DirTree::new(src.path(), dst.path()).expect("Unable to create tree");
            sort(&mut tree.root);

            assert_eq!(
                tree.root,
                TreeNode {
                    path: PathBuf::from(""),
                    presence: Presence::Both,
                    kind: FileType::Dir,
                    children: Some(vec![
                        TreeNode {
                            path: PathBuf::from("a.txt"),
                            presence: Presence::Both,
                            kind: FileType::File,
                            children: None
                        },
                        TreeNode {
                            path: PathBuf::from("b.txt"),
                            presence: Presence::Src,
                            kind: FileType::File,
                            children: None
                        },
                        TreeNode {
                            path: PathBuf::from("bin"),
                            presence: Presence::Both,
                            kind: FileType::Dir,
                            children: Some(vec![
                                TreeNode {
                                    path: PathBuf::from("bin/c.txt"),
                                    presence: Presence::Both,
                                    kind: FileType::File,
                                    children: None
                                },
                                TreeNode {
                                    path: PathBuf::from("bin/d.txt"),
                                    presence: Presence::Both,
                                    kind: FileType::File,
                                    children: None
                                },
                            ])
                        },
                        TreeNode {
                            path: PathBuf::from("e.txt"),
                            presence: Presence::Dst,
                            kind: FileType::File,
                            children: None
                        },
                        TreeNode {
                            path: PathBuf::from("target"),
                            presence: Presence::Dst,
                            kind: FileType::Dir,
                            children: Some(vec![])
                        },
                    ])
                }
            )
        }

        #[test]
        fn test_tree_iteration() {
            let (src, dst) = generate_tree();

            let mut tree = DirTree::new(src.path(), dst.path()).expect("Unable to create tree");
            sort(&mut tree.root);

            assert_eq!(
                tree.iter()
                    .map(|e| e.node.path.display().to_string())
                    .collect::<Vec<String>>(),
                vec![
                    "",
                    "a.txt",
                    "b.txt",
                    "bin",
                    "e.txt",
                    "target",
                    "bin/c.txt",
                    "bin/d.txt"
                ]
            )
        }
    }
}
