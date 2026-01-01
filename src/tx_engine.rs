use crate::models::{InodeMetadata, Extent};
use crate::log_manager::{LogManager, BlockDevice, crc32};
use crate::common::{DbfsResult, DbfsError};
use jammdb::DB;
use alloc::vec::Vec;

pub struct TransactionEngine<D: BlockDevice> {
    db: DB,
    log_manager: LogManager<D>,
}

impl<D: BlockDevice> TransactionEngine<D> {
    pub fn new(db: DB, log_manager: LogManager<D>) -> Self {
        Self { db, log_manager }
    }

    pub fn write_file_transactional(&mut self, ino: u64, offset: u64, data: &[u8]) -> DbfsResult<()> {
        // --- 步骤 1: 数据持久化 (数据层先走) ---
        // 即使这一步写完后断电，因为没有索引，数据在重启后是“不可见”的。
        let p_ptr = self.log_manager.append_data(data)?;

        // --- 步骤 2: 开启数据库事务 (索引层后跟) ---
        let tx = self.db.begin_batch();
        let bucket = tx.get_bucket("inodes").map_err(|_| DbfsError::NotFound)?;

        // --- 步骤 3: 读取-修改-写回 (Read-Modify-Write) ---
        let ino_key = ino.to_be_bytes();
        let kv = bucket.get(&ino_key).ok_or(DbfsError::NotFound)?;
        
        let mut meta: InodeMetadata = deserialize(kv.kv().value())?;
        
        // 增加新的映射关系
        meta.extents.push(Extent {
            logical_off: offset,
            physical_ptr: p_ptr,
            len: data.len() as u64,
            crc: crc32(data),
        });
        meta.size = core::cmp::max(meta.size, offset + data.len() as u64);
        // meta.mtime = now(); // TODO: 实现获取当前时间的逻辑

        // 将新的元数据覆盖写入数据库
        bucket.put(ino_key, serialize(&meta)?)?;

        // --- 故障注入测试点 ---
        // if cfg!(feature = "crash_test") { panic!("Simulated Crash before commit!"); }

        // --- 步骤 4: 原子提交 (The Commit) ---
        // 这是唯一的故障切换点。jammdb 保证此操作要么全成功，要么全失败。
        tx.commit().map_err(|_| DbfsError::Io)?;

        Ok(())
    }

    /// 获取 Inode 元数据
    pub fn get_metadata(&self, ino: u64) -> DbfsResult<InodeMetadata> {
        let tx = self.db.tx(false).map_err(|_| DbfsError::Io)?;
        let bucket = tx.get_bucket("inodes").map_err(|_| DbfsError::NotFound)?;
        let kv = bucket.get(&ino.to_be_bytes()).ok_or(DbfsError::NotFound)?;
        deserialize(kv.kv().value())
    }

    /// 根据 Extents 从日志读取数据
    pub fn read_from_log(&self, meta: &InodeMetadata, offset: u64, buf: &mut [u8]) -> DbfsResult<usize> {
        if offset >= meta.size {
            return Ok(0);
        }

        let mut bytes_read = 0;
        let mut current_offset = offset;
        let mut buf_pos = 0;

        while buf_pos < buf.len() && current_offset < meta.size {
            // 查找包含 current_offset 的 extent
            let extent = meta.extents.iter().find(|e| {
                current_offset >= e.logical_off && current_offset < e.logical_off + e.len
            });

            if let Some(e) = extent {
                let off_in_extent = current_offset - e.logical_off;
                let len_in_extent = core::cmp::min(
                    (e.len - off_in_extent) as usize,
                    buf.len() - buf_pos
                );

                let physical_pos = e.physical_ptr + off_in_extent;
                self.log_manager.read_data(physical_pos, &mut buf[buf_pos..buf_pos + len_in_extent])?;

                bytes_read += len_in_extent;
                buf_pos += len_in_extent;
                current_offset += len_in_extent as u64;
            } else {
                // 如果没找到 extent，说明是空洞 (hole)
                // TODO: 寻找下一个 extent 的开始位置，中间补 0
                break;
            }
        }

        Ok(bytes_read)
    }
}

// 序列化辅助函数 (暂用 serde_json，后续可替换为更高效的 postcard 等)
fn serialize<T: serde::Serialize>(obj: &T) -> DbfsResult<Vec<u8>> {
    serde_json::to_vec(obj).map_err(|_| DbfsError::Other)
}

// 反序列化辅助函数
fn deserialize<'a, T: serde::Deserialize<'a>>(data: &'a [u8]) -> DbfsResult<T> {
    serde_json::from_slice(data).map_err(|_| DbfsError::Other)
}
