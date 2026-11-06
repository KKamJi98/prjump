//! prjump: open the GitHub or Bitbucket Cloud pull request page for a branch.
//!
//! The binary is `prj`. This library exists so the pieces can be unit tested.

pub mod browser;
pub mod cli;
pub mod clipboard;
pub mod config;
pub mod error;
pub mod git;
pub mod head;
pub mod provider;
pub mod pystr;
pub mod remote;
