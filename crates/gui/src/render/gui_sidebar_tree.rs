//! Project file-tree construction for the sidebar.  The tree model lives in
//! the kernel so every frontend renders the same projection.

pub(crate) use magma::kernel::project::tree::{build, collect_dirs, FileNode};
