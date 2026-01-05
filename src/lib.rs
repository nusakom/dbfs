#![feature(error_in_core)]
#![cfg_attr(not(test), no_std)]
extern crate alloc;

// Common modules (no VFS dependency)
#[cfg(feature = "rvfs")]
mod attr;
mod common;
mod inode_common;
#[cfg(feature = "rvfs")]
mod link;

// New RVFS2 support
#[cfg(feature = "rvfs2")]
pub mod rvfs2;

// RVFS2 Demo - Minimal proof of concept
#[cfg(feature = "rvfs2_demo")]
pub mod rvfs2_demo;

// Alien Integration - Phase 1: Basic filesystem (no transactions)
#[cfg(feature = "alien_integration")]
pub mod alien_integration;

// DBFS Transactional Core
pub mod operation;
pub mod wal;
pub mod transaction;

#[cfg(test)]
mod dbfs_test;

// Common DBFS functions for both old and new RVFS
#[cfg(feature = "rvfs")]
mod common;

mod fs_common;

// Old RVFS modules (only compile when rvfs feature is available)
#[cfg(feature = "rvfs")]
mod dir;
#[cfg(feature = "rvfs")]
mod file;
#[cfg(feature = "rvfs")]
mod fs_type;
#[cfg(feature = "rvfs")]
mod inode;

use alloc::{alloc::alloc, sync::Arc};
use core::{
    alloc::Layout,
    ops::{Deref, DerefMut},
};

use buddy_system_allocator::LockedHeap;

#[cfg(feature = "rvfs")]
pub use fs_type::DBFS;

use jammdb::DB;
use log::error;
use spin::Once;

#[cfg(feature = "dbop")]
pub mod extend;

#[cfg(feature = "dbop")]
pub mod models;

#[cfg(feature = "dbop")]
pub mod log_manager;

#[cfg(feature = "dbop")]
pub mod tx_engine;

#[cfg(feature = "dbop")]
pub mod rvfs_adapter;

#[cfg(all(test, feature = "dbop"))]
mod rvfs_test;
#[cfg(feature = "fuse")]
pub use file::FLAG;

#[cfg(feature = "fuse")]
pub mod fuse;

#[cfg(feature = "fuse")]
extern crate std;

struct SafeDb(DB);

impl Deref for SafeDb {
    type Target = DB;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SafeDb {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

unsafe impl Sync for SafeDb {}
unsafe impl Send for SafeDb {}

static DB: Once<Arc<SafeDb>> = Once::new();

/// Initialize the global DBFS database
pub fn init_dbfs(db: DB) {
    DB.call_once(|| Arc::new(SafeDb(db)));
}

fn clone_db() -> Arc<SafeDb> {
    DB.get().unwrap().clone()
}

#[macro_export]
macro_rules! u32 {
    ($x:expr) => {
        u32::from_be_bytes($x.try_into().unwrap())
    };
}

#[macro_export]
macro_rules! u16 {
    ($x:expr) => {
        u16::from_be_bytes($x.try_into().unwrap())
    };
}

#[macro_export]
macro_rules! usize {
    ($x:expr) => {
        usize::from_be_bytes($x.try_into().unwrap())
    };
}
#[macro_export]
macro_rules! u64 {
    ($x:expr) => {
        u64::from_be_bytes($x.try_into().unwrap())
    };
}

#[macro_export]
macro_rules! dbfs_time_spec {
    ($x:expr) => {
        crate::common::DbfsTimeSpec::from($x)
    };
}

#[cfg(feature = "sli512")]
pub const SLICE_SIZE: usize = 512;

#[cfg(feature = "sli1k")]
pub const SLICE_SIZE: usize = 1024;

#[cfg(feature = "sli4k")]
pub const SLICE_SIZE: usize = 4096;

#[cfg(feature = "sli8k")]
pub const SLICE_SIZE: usize = 8192;

#[cfg(feature = "sli32k")]
pub const SLICE_SIZE: usize = 8192 * 2 * 2;

static BUDDY_ALLOCATOR: LockedHeap<32> = LockedHeap::empty();
const MAX_BUF_SIZE: usize = 8 * 1024 * 1024; // 8MB

pub const BUCKET_DATA_SIZE: usize = 128 * 1024 * 1024; // 512

fn init_cache() {
    error!("alloc {}MB for cache", 8);
    unsafe {
        let ptr = alloc(Layout::from_size_align_unchecked(MAX_BUF_SIZE, 8));
        BUDDY_ALLOCATOR.lock().init(ptr as usize, MAX_BUF_SIZE);
    };
    error!("alloc ok");
}

fn copy_data(src: *const u8, dest: *mut u8, len: usize) {
    if src as usize % 16 == 0 && dest as usize % 16 == 0 && len % 16 == 0 {
        unsafe {
            (dest as *mut u128).copy_from_nonoverlapping(src as *const u128, len / 16);
        }
    } else if src as usize % 8 == 0 && dest as usize % 8 == 0 && len % 8 == 0 {
        unsafe {
            (dest as *mut u64).copy_from_nonoverlapping(src as *const u64, len / 8);
        }
    } else if src as usize % 4 == 0 && dest as usize % 4 == 0 && len % 4 == 0 {
        unsafe {
            (dest as *mut u32).copy_from_nonoverlapping(src as *const u32, len / 4);
        }
    } else if src as usize % 2 == 0 && dest as usize % 2 == 0 && len % 2 == 0 {
        unsafe {
            (dest as *mut u16).copy_from_nonoverlapping(src as *const u16, len / 2);
        }
    } else {
        unsafe {
            dest.copy_from_nonoverlapping(src, len);
        }
    }
}
