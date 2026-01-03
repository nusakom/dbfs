//! Common inode-related definitions shared between old RVFS and new RVFS2

use core::sync::atomic::{AtomicUsize, Ordering};

/// Global inode number counter
pub static DBFS_INODE_NUMBER: AtomicUsize = AtomicUsize::new(1);
