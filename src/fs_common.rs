//! Common DBFS functions shared between old RVFS and new RVFS2
//!
//! This module provides core functionality that doesn't depend on
//! any specific VFS API.

use alloc::vec::Vec;
use jammdb::Data;

use crate::{
    clone_db,
    common::{DbfsFsStat, DbfsPermission, DbfsResult, DbfsTimeSpec},
    inode_common::DBFS_INODE_NUMBER,
    u32, u64, usize,
};

/// Initialize the root inode
///
/// This is a simplified version that works with the new vfscore API
pub fn dbfs_common_root_inode(uid: u32, gid: u32, ctime: DbfsTimeSpec) -> DbfsResult<usize> {
    let db = clone_db();
    let tx = db.tx(true)?;

    if tx.get_bucket(1usize.to_be_bytes()).is_err() {
        // Create root directory inode
        let permission = DbfsPermission::from_bits_truncate(0o755) | DbfsPermission::S_IFDIR;
        let new_inode = tx.create_bucket(1usize.to_be_bytes()).unwrap();
        let old = DBFS_INODE_NUMBER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        assert_eq!(old, 1);

        new_inode
            .put("mode", permission.bits().to_be_bytes())
            .unwrap();
        new_inode.put("hard_links", 2u32.to_be_bytes()).unwrap();
        new_inode.put("uid", uid.to_be_bytes()).unwrap();
        new_inode.put("gid", gid.to_be_bytes()).unwrap();

        // Set timestamps
        new_inode.put("atime", ctime.to_be_bytes()).unwrap();
        new_inode.put("mtime", ctime.to_be_bytes()).unwrap();
        new_inode.put("ctime", ctime.to_be_bytes()).unwrap();
    }

    drop(tx);
    Ok(1)
}

/// Unmount DBFS
pub fn dbfs_common_umount() -> DbfsResult<()> {
    // In the new architecture, unmounting is handled by the VFS layer
    // This function is kept for compatibility but does nothing
    Ok(())
}

/// Get filesystem statistics
pub fn dbfs_common_statfs(
    _total_blocks: u64,
    _free_blocks: u64,
    _total_files: u64,
    _free_files: u64,
    _block_size: u32,
) -> DbfsResult<DbfsFsStat> {
    // Return default filesystem statistics
    // In a real implementation, these would be calculated from the database
    Ok(DbfsFsStat {
        f_bsize: 4096,
        f_frsize: 4096,
        f_blocks: 0,
        f_bfree: 0,
        f_bavail: 0,
        f_files: 0,
        f_ffree: 0,
        f_favail: 0,
        f_fsid: 0,
        f_flag: 0,
        f_namemax: 255,
        name: [0; 32],
    })
}
