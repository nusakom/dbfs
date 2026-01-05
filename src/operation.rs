use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionOperation {
    Write {
        ino: usize,
        offset: u64,
        data: Vec<u8>,
    },
    Create {
        parent_ino: usize,
        name: String,
        uid: u32,
        gid: u32,
        perm: u32, // DbfsPermission bits
        dev: Option<u32>,
    },
    Delete {
        parent_ino: usize,
        name: String,
    },
    Rename {
        old_parent_ino: usize,
        old_name: String,
        new_parent_ino: usize,
        new_name: String,
    },
    Mkdir {
        parent_ino: usize,
        name: String,
        uid: u32,
        gid: u32,
        perm: u32,
    },
    Truncate {
        ino: usize,
        length: u64,
    },
}

impl TransactionOperation {
    /// Apply the operation to the underlying filesystem.
    pub fn apply(&self) -> Result<(), String> {
        use crate::common::DbfsTimeSpec;
        // In a real system, we'd take the current time or a timestamp from the operation.
        let now = DbfsTimeSpec { sec: 0, nsec: 0 };

        match self {
            TransactionOperation::Write { ino, offset, data } => {
                crate::common::dbfs_write(*ino, data, *offset)
                    .map_err(|e| alloc::format!("Write error: {:?}", e))?;
            }
            TransactionOperation::Create { parent_ino, name, uid, gid, perm, dev } => {
                crate::common::dbfs_create(
                    *parent_ino,
                    name,
                    *uid,
                    *gid,
                    now,
                    crate::common::DbfsPermission::from_bits_truncate(*perm),
                    None,
                    *dev,
                ).map_err(|e| alloc::format!("Create error: {:?}", e))?;
            }
            TransactionOperation::Delete { parent_ino, name } => {
                crate::common::dbfs_unlink(0, 0, *parent_ino, name, None, now)
                    .map_err(|e| alloc::format!("Delete error: {:?}", e))?;
            }
            TransactionOperation::Rename { old_parent_ino, old_name, new_parent_ino, new_name } => {
                crate::common::dbfs_rename(
                    *old_parent_ino,
                    old_name,
                    *new_parent_ino,
                    new_name,
                    0, // flags
                    now,
                ).map_err(|e| alloc::format!("Rename error: {:?}", e))?;
            }
            TransactionOperation::Mkdir { parent_ino, name, uid, gid, perm } => {
                crate::common::dbfs_create(
                    *parent_ino,
                    name,
                    *uid,
                    *gid,
                    now,
                    crate::common::DbfsPermission::from_bits_truncate(*perm | crate::common::DbfsPermission::S_IFDIR.bits()),
                    None,
                    None,
                ).map_err(|e| alloc::format!("Mkdir error: {:?}", e))?;
            }
            TransactionOperation::Truncate { ino, length } => {
                crate::common::dbfs_truncate(*ino, *length, now)
                    .map_err(|e| alloc::format!("Truncate error: {:?}", e))?;
            }
        }
        Ok(())
    }
}
