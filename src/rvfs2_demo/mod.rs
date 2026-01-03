//! DBFS RVFS2 Demo - Simplified Proof of Concept
//!
//! This is a minimal implementation to prove that DBFS can work with vfscore.
//! It demonstrates:
//! - DbfsFsType can be registered
//! - mount() succeeds and returns root dentry
//! - root inode exists
//! - lookup("hello") returns a fixed inode
//! - read_at() returns fixed content

mod dentry;
mod fstype;
mod inode;
mod superblock;

pub use fstype::DbfsFsType;
