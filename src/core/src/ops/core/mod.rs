//! Implements the low level details of artid's operations.
//!
//! These details are divided in two important elements:
//!
//!   - The DirTree that holds the information about the how the filesystem looks
//!   - The CopyModel that can be used as the model of most operations

mod model;
mod tree;

pub use self::{
    model::{Actions, CopyAction, CopyModel, MultipleCopyModel},
    tree::{DirTree, Direction, FileType, Presence, TreeIter, TreeIterNode, TreeNode},
};
