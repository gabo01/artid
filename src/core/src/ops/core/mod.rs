//! Implements the low level details of artid's operations.
//!
//! These details are divided in two important elements:
//!
//!   - The DirTree that holds the information about the how the filesystem looks
//!   - The CopyModel that can be used as the model of most operations

pub mod filesystem;
pub mod model;
pub mod tree;
