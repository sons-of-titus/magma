//! Built-in VCS backends: Git, Mercurial (hg), Subversion (svn), Jujutsu (jj).
//!
//! Each backend is a zero-size struct that shells out to the VCS CLI.
//! All methods receive the repo root directory detected by `VcState::detect_for`.

mod git;
mod hg;
mod jj;
mod parsers;
mod shell_template;
mod svn;

pub use git::GitBackend;
pub use hg::HgBackend;
pub use jj::JjBackend;
pub use parsers::{parse_branch_lines, parse_porcelain_z, parse_stash_lines, split_hunks};
pub use shell_template::ShellTemplateBackend;
pub use svn::SvnBackend;

