use serde::{Serialize, Deserialize};
use alloc::vec::Vec;

/// 物理数据块描述符
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Extent {
    pub logical_off: u64,  // 文件内部的逻辑偏移
    pub physical_ptr: u64, // 磁盘数据区的绝对偏移
    pub len: u64,          // 数据长度
    pub crc: u32,          // 用于崩溃后校验数据完整性
}

/// 存储在 jammdb Value 中的 Inode 元数据
#[derive(Serialize, Deserialize, Debug)]
pub struct InodeMetadata {
    pub ino: u64,
    pub size: u64,
    pub mode: u32,         // 权限与类型
    pub nlink: u32,        // 硬链接计数
    pub extents: Vec<Extent>, // 物理块映射表（索引核心）
    pub atime: i64,
    pub mtime: i64,
}
