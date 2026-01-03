//! DBFS Alien Integration
//!
//! Phase 1: 基础文件系统功能 (无事务性)
//!
//! 目标:
//! - ✅ 可以在 Alien OS 中注册和挂载
//! - ✅ 支持基本的 inode 操作: lookup, create, mkdir, read_at, write_at, unlink
//! - ✅ 支持基本的 dentry 操作: insert, remove, parent
//! - ✅ 可以像 ramfs 一样使用
//!
//! ❌ 不实现: BEGIN / COMMIT / 事务性 / ioctl

mod dentry;
mod fstype;
mod inode;
mod superblock;

pub use fstype::DbfsFsType;
