//! RVFS2-specific common functions
//!
//! These are re-implementations of common DBFS operations that work
//! with the new vfscore API, independent of the old rvfs crate.

use alloc::{string::String, vec::Vec};
use jammdb::{Bucket, Data};
use log::debug;

use crate::{
    clone_db,
    common::{
        DbfsAttr, DbfsFileType, DbfsPermission, DbfsResult, DbfsTimeSpec, RENAME_EXCHANGE,
    },
    inode_common::DBFS_INODE_NUMBER,
    u32, u64, usize,
};

/// Read data from a file
pub fn dbfs_read(number: usize, buf: &mut [u8], offset: u64) -> DbfsResult<usize> {
    let db = clone_db();
    let tx = db.tx(true)?;

    let bucket = tx.get_bucket(number.to_be_bytes())?;
    let data_key = format!("data_{}", offset / 4096);

    match bucket.get(data_key.as_bytes()) {
        Some(Data::KeyValue(kv)) => {
            let value = kv.value();
            let bytes_to_read = core::cmp::min(buf.len(), value.len());
            buf[..bytes_to_read].copy_from_slice(&value[..bytes_to_read]);
            Ok(bytes_to_read)
        }
        _ => Ok(0),
    }
}

/// Write data to a file
pub fn dbfs_write(number: usize, buf: &[u8], offset: u64) -> DbfsResult<usize> {
    let db = clone_db();
    let tx = db.tx(true)?;

    let bucket = tx.get_bucket(number.to_be_bytes())?;
    let data_key = format!("data_{}", offset / 4096);

    bucket.put(data_key.as_bytes(), buf)?;
    tx.commit()?;

    // Update file size
    let current_size = bucket
        .get_kv("size")
        .map(|kv| crate::u64!(kv.value()))
        .unwrap_or(0);

    let new_size = core::cmp::max(current_size, offset as u64 + buf.len() as u64);
    bucket.put("size", new_size.to_be_bytes())?;

    Ok(buf.len())
}

/// Get file attributes
pub fn dbfs_get_attr(number: usize) -> DbfsResult<DbfsAttr> {
    let db = clone_db();
    let tx = db.tx(true)?;

    let bucket = tx.get_bucket(number.to_be_bytes())?;

    let mode = bucket
        .get_kv("mode")
        .map(|kv| DbfsPermission::from_bits_truncate(crate::u16!(kv.value())))
        .unwrap_or(DbfsPermission::from_bits_truncate(0o755));

    let size = bucket
        .get_kv("size")
        .map(|kv| crate::u64!(kv.value()))
        .unwrap_or(0);

    let uid = bucket
        .get_kv("uid")
        .map(|kv| crate::u32!(kv.value()))
        .unwrap_or(0);

    let gid = bucket
        .get_kv("gid")
        .map(|kv| crate::u32!(kv.value()))
        .unwrap_or(0);

    let atime = bucket
        .get_kv("atime")
        .map(|kv| DbfsTimeSpec::from(kv.value()))
        .unwrap_or_default();

    let mtime = bucket
        .get_kv("mtime")
        .map(|kv| DbfsTimeSpec::from(kv.value()))
        .unwrap_or_default();

    let ctime = bucket
        .get_kv("ctime")
        .map(|kv| DbfsTimeSpec::from(kv.value()))
        .unwrap_or_default();

    let hard_links = bucket
        .get_kv("hard_links")
        .map(|kv| crate::u32!(kv.value()))
        .unwrap_or(1);

    Ok(DbfsAttr {
        ino: number,
        size,
        mode,
        uid,
        gid,
        atime,
        mtime,
        ctime,
        hard_links,
    })
}

/// Truncate a file to a specific size
pub fn dbfs_truncate(number: usize, size: u64) -> DbfsResult<()> {
    let db = clone_db();
    let tx = db.tx(true)?;

    let bucket = tx.get_bucket(number.to_be_bytes())?;

    // Update size
    bucket.put("size", size.to_be_bytes())?;

    // Remove data blocks beyond the new size
    let block_size = 4096u64;
    let start_block = (size + block_size - 1) / block_size;

    // Find and remove blocks
    let mut blocks_to_remove = Vec::new();
    bucket.cursor().for_each(|data| {
        if let Data::KeyValue(kv) = data {
            let key = core::str::from_utf8(kv.key()).unwrap_or("");
            if key.starts_with("data_") {
                if let Ok(block_num) = key.trim_start_matches("data_").parse::<u64>() {
                    if block_num >= start_block {
                        blocks_to_remove.push(key.to_string());
                    }
                }
            }
        }
    });

    for key in blocks_to_remove {
        bucket.delete(key.as_bytes())?;
    }

    tx.commit()?;
    Ok(())
}

/// Create a new file or directory
pub fn dbfs_create(
    parent: usize,
    name: &str,
    file_type: DbfsFileType,
    uid: u32,
    gid: u32,
    mode: DbfsPermission,
) -> DbfsResult<usize> {
    let db = clone_db();
    let tx = db.tx(true)?;

    // Get parent bucket
    let parent_bucket = tx.get_bucket(parent.to_be_bytes())?;

    // Check if name already exists
    if parent_bucket.get(name.as_bytes()).is_some() {
        return Err(DbfsResult::Err("File exists"));
    }

    // Allocate new inode number
    let ino = DBFS_INODE_NUMBER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);

    // Create new inode bucket
    let new_inode = tx.create_bucket(ino.to_be_bytes())?;

    // Set attributes
    let mut final_mode = mode;
    match file_type {
        DbfsFileType::File => final_mode |= DbfsPermission::S_IFREG,
        DbfsFileType::Dir => final_mode |= DbfsPermission::S_IFDIR,
        DbfsFileType::Symlink => final_mode |= DbfsPermission::S_IFLNK,
    }

    let now = DbfsTimeSpec::default();

    new_inode.put("mode", final_mode.bits().to_be_bytes())?;
    new_inode.put("size", 0u64.to_be_bytes())?;
    new_inode.put("hard_links", 1u32.to_be_bytes())?;
    new_inode.put("uid", uid.to_be_bytes())?;
    new_inode.put("gid", gid.to_be_bytes())?;
    new_inode.put("atime", now.to_be_bytes())?;
    new_inode.put("mtime", now.to_be_bytes())?;
    new_inode.put("ctime", now.to_be_bytes())?;

    // Add to parent directory
    parent_bucket.put(name.as_bytes(), ino.to_be_bytes())?;

    // Update parent's hard_links count if it's a directory
    if file_type == DbfsFileType::Dir {
        let parent_links = parent_bucket
            .get_kv("hard_links")
            .map(|kv| crate::u32!(kv.value()))
            .unwrap_or(2);
        parent_bucket.put("hard_links", (parent_links + 1).to_be_bytes())?;
    }

    tx.commit()?;
    Ok(ino)
}

/// Lookup a file in a directory
pub fn dbfs_lookup(parent: usize, name: &str) -> DbfsResult<Option<usize>> {
    let db = clone_db();
    let tx = db.tx(false)?;

    let parent_bucket = tx.get_bucket(parent.to_be_bytes())?;

    match parent_bucket.get(name.as_bytes()) {
        Some(Data::KeyValue(kv)) => {
            let ino = crate::usize!(kv.value());
            Ok(Some(ino))
        }
        _ => Ok(None),
    }
}

/// Create a hard link
pub fn dbfs_link(old_parent: usize, old_name: &str, new_parent: usize, new_name: &str) -> DbfsResult<()> {
    let db = clone_db();
    let tx = db.tx(true)?;

    // Get old entry
    let old_bucket = tx.get_bucket(old_parent.to_be_bytes())?;
    let old_entry = old_bucket.get(old_name.as_bytes());

    if old_entry.is_none() {
        return Err(DbfsResult::Err("Old file not found"));
    }

    let ino = crate::usize!(old_entry.unwrap().value());

    // Get new parent bucket
    let new_bucket = tx.get_bucket(new_parent.to_be_bytes())?;

    // Check if new name already exists
    if new_bucket.get(new_name.as_bytes()).is_some() {
        return Err(DbfsResult::Err("New file already exists"));
    }

    // Add link
    new_bucket.put(new_name.as_bytes(), ino.to_be_bytes())?;

    // Increment hard_links count
    let inode_bucket = tx.get_bucket(ino.to_be_bytes())?;
    let links = inode_bucket
        .get_kv("hard_links")
        .map(|kv| crate::u32!(kv.value()))
        .unwrap_or(1);
    inode_bucket.put("hard_links", (links + 1).to_be_bytes())?;

    tx.commit()?;
    Ok(())
}

/// Unlink (delete) a file
pub fn dbfs_unlink(parent: usize, name: &str) -> DbfsResult<()> {
    let db = clone_db();
    let tx = db.tx(true)?;

    let parent_bucket = tx.get_bucket(parent.to_be_bytes())?;

    // Get inode number
    let entry = parent_bucket.get(name.as_bytes());
    if entry.is_none() {
        return Err(DbfsResult::Err("File not found"));
    }

    let ino = crate::usize!(entry.unwrap().value());

    // Remove entry from parent
    parent_bucket.delete(name.as_bytes())?;

    // Decrement hard_links count
    let inode_bucket = tx.get_bucket(ino.to_be_bytes())?;
    let links = inode_bucket
        .get_kv("hard_links")
        .map(|kv| crate::u32!(kv.value()))
        .unwrap_or(1);

    if links <= 1 {
        // Last link, delete the inode
        tx.delete_bucket(ino.to_be_bytes())?;
    } else {
        inode_bucket.put("hard_links", (links - 1).to_be_bytes())?;
    }

    tx.commit()?;
    Ok(())
}

/// Read directory entries
pub fn dbfs_readdir(parent: usize) -> DbfsResult<Vec<(String, usize)>> {
    let db = clone_db();
    let tx = db.tx(false)?;

    let bucket = tx.get_bucket(parent.to_be_bytes())?;
    let mut entries = Vec::new();

    bucket.cursor().for_each(|data| {
        if let Data::KeyValue(kv) = data {
            let key = core::str::from_utf8(kv.key()).ok();
            if let Some(name) = key {
                // Skip non-entries
                if !name.starts_with("data_")
                    && !name.starts_with("mode")
                    && !name.starts_with("size")
                    && !name.starts_with("uid")
                    && !name.starts_with("gid")
                    && !name.starts_with("atime")
                    && !name.starts_with("mtime")
                    && !name.starts_with("ctime")
                    && !name.starts_with("hard_links")
                {
                    let ino = crate::usize!(kv.value());
                    entries.push((name.to_string(), ino));
                }
            }
        }
    });

    Ok(entries)
}

/// Rename a file
pub fn dbfs_rename(
    old_parent: usize,
    old_name: &str,
    new_parent: usize,
    new_name: &str,
    _flags: u32,
) -> DbfsResult<()> {
    let db = clone_db();
    let tx = db.tx(true)?;

    let old_bucket = tx.get_bucket(old_parent.to_be_bytes())?;

    // Get old entry
    let old_entry = old_bucket.get(old_name.as_bytes());
    if old_entry.is_none() {
        return Err(DbfsResult::Err("Old file not found"));
    }

    let ino = crate::usize!(old_entry.unwrap().value());

    // Remove old entry
    old_bucket.delete(old_name.as_bytes())?;

    // Add new entry
    if old_parent == new_parent {
        // Same directory
        old_bucket.put(new_name.as_bytes(), ino.to_be_bytes())?;
    } else {
        // Different directory
        let new_bucket = tx.get_bucket(new_parent.to_be_bytes())?;
        new_bucket.put(new_name.as_bytes(), ino.to_be_bytes())?;
    }

    tx.commit()?;
    Ok(())
}

/// Create a symbolic link
pub fn dbfs_symlink(parent: usize, name: &str, target: &str, uid: u32, gid: u32) -> DbfsResult<usize> {
    let db = clone_db();
    let tx = db.tx(true)?;

    // Allocate new inode number
    let ino = DBFS_INODE_NUMBER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);

    // Create new symlink inode
    let new_inode = tx.create_bucket(ino.to_be_bytes())?;

    let mode = DbfsPermission::S_IFLNK | DbfsPermission::from_bits_truncate(0o777);
    let now = DbfsTimeSpec::default();

    new_inode.put("mode", mode.bits().to_be_bytes())?;
    new_inode.put("size", target.len() as u64.to_be_bytes())?;
    new_inode.put("hard_links", 1u32.to_be_bytes())?;
    new_inode.put("uid", uid.to_be_bytes())?;
    new_inode.put("gid", gid.to_be_bytes())?;
    new_inode.put("atime", now.to_be_bytes())?;
    new_inode.put("mtime", now.to_be_bytes())?;
    new_inode.put("ctime", now.to_be_bytes())?;
    new_inode.put("symlink_target", target.as_bytes())?;

    // Add to parent directory
    let parent_bucket = tx.get_bucket(parent.to_be_bytes())?;
    parent_bucket.put(name.as_bytes(), ino.to_be_bytes())?;

    tx.commit()?;
    Ok(ino)
}

/// Read symbolic link target
pub fn dbfs_readlink(ino: usize) -> DbfsResult<String> {
    let db = clone_db();
    let tx = db.tx(false)?;

    let bucket = tx.get_bucket(ino.to_be_bytes())?;

    match bucket.get("symlink_target") {
        Some(Data::KeyValue(kv)) => {
            let target = core::str::from_utf8(kv.value())
                .map_err(|_| DbfsResult::Err("Invalid symlink target"))?;
            Ok(target.to_string())
        }
        _ => Err(DbfsResult::Err("Not a symlink")),
    }
}

/// Remove a directory
pub fn dbfs_rmdir(parent: usize, name: &str) -> DbfsResult<()> {
    let db = clone_db();
    let tx = db.tx(true)?;

    let parent_bucket = tx.get_bucket(parent.to_be_bytes())?;

    // Get inode number
    let entry = parent_bucket.get(name.as_bytes());
    if entry.is_none() {
        return Err(DbfsResult::Err("Directory not found"));
    }

    let ino = crate::usize!(entry.unwrap().value());

    // Check if directory is empty
    let dir_bucket = tx.get_bucket(ino.to_be_bytes())?;
    let mut is_empty = true;
    dir_bucket.cursor().for_each(|data| {
        if let Data::KeyValue(kv) = data {
            let key = core::str::from_utf8(kv.key()).unwrap_or("");
            if !key.starts_with("mode")
                && !key.starts_with("size")
                && !key.starts_with("uid")
                && !key.starts_with("gid")
                && !key.starts_with("atime")
                && !key.starts_with("mtime")
                && !key.starts_with("ctime")
                && !key.starts_with("hard_links")
            {
                is_empty = false;
            }
        }
    });

    if !is_empty {
        return Err(DbfsResult::Err("Directory not empty"));
    }

    // Remove entry from parent
    parent_bucket.delete(name.as_bytes())?;

    // Delete the directory inode
    tx.delete_bucket(ino.to_be_bytes())?;

    // Update parent's hard_links count
    let parent_links = parent_bucket
        .get_kv("hard_links")
        .map(|kv| crate::u32!(kv.value()))
        .unwrap_or(2);
    parent_bucket.put("hard_links", (parent_links - 1).to_be_bytes())?;

    tx.commit()?;
    Ok(())
}
