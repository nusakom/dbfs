# DBFS RVFS2 适配说明

## 概述

DBFS 正在适配新的 RVFS (vfscore) API。新的 RVFS 使用了更加面向对象的 trait 设计，替代了旧的函数指针结构体。

## 架构关系：Git 依赖 + 明确解耦

**重要**：DBFS 和 RVFS 是两个独立的 Git 仓库，通过明确的依赖关系协作：

- **RVFS 仓库**: https://github.com/Godones/rvfs.git
  - 提供虚拟文件系统框架 (vfscore)
  - 定义标准 trait 接口 (VfsInode, VfsFile, VfsSuperBlock, VfsFsType)
  - 包含多个文件系统实现示例 (ramfs, fat, ext4, etc.)

- **DBFS 仓库**: https://github.com/Godones/dbfs2.git
  - 独立的数据库文件系统实现
  - 通过 Cargo.toml 明确依赖 RVFS
  - 实现 RVFS 定义的 trait 接口
  - 保持业务逻辑独立性

```toml
# DBFS 的 Cargo.toml - 明确的 Git 依赖
[dependencies]
vfscore = { git = "https://github.com/Godones/rvfs.git", optional = true }
```

**不是**：
- ❌ 代码拷贝
- ❌ 私有分支
- ❌ 紧耦合

**而是**：
- ✅ 通过 Git 依赖明确解耦
- ✅ API 层面适配（实现 trait）
- ✅ 独立版本管理

### 依赖关系图

```
┌─────────────────────────────────────────────────────────────┐
│                        应用层                                │
│                  (应用代码 / 系统调用)                         │
└───────────────────────────┬─────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                   RVFS (vfscore)                             │
│              https://github.com/Godones/rvfs.git             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  VfsFsType  VfsSuperBlock  VfsInode  VfsFile       │    │
│  │     (Trait 定义 - 抽象接口)                          │    │
│  └─────────────────────────────────────────────────────┘    │
└───────────────────────────┬─────────────────────────────────┘
                            │ Git 依赖
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                     DBFS (独立仓库)                           │
│               https://github.com/Godones/dbfs2.git           │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  DbfsFsType  DbfsSuperBlock  DbfsInode             │    │
│  │     (实现 RVFS trait - 具体实现)                      │    │
│  └─────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  dbfs_common_*  (业务逻辑 - 与 jammdb 数据库交互)     │    │
│  └─────────────────────────────────────────────────────┘    │
└───────────────────────────┬─────────────────────────────────┘
                            ▼
                     ┌──────────┐
                     │  jammdb  │
                     │  (数据库) │
                     └──────────┘
```

### 关键设计原则

1. **接口与实现分离**
   - RVFS 定义接口 (trait)
   - DBFS 提供实现 (impl trait for Dbfs*)

2. **通过 Git 依赖解耦**
   - DBFS 的 Cargo.toml: `vfscore = { git = "..." }`
   - 更新 RVFS: `cargo update`
   - 版本独立演进

3. **可替换性**
   - 任何实现 RVFS trait 的文件系统都可以挂载
   - ramfs, fat, ext4, dbfs 都实现同一套接口

## 架构变化

### 旧 RVFS API
- 使用函数指针结构体：`InodeOps`, `FileOps`, `SuperBlockOps`
- 函数式回调风格
- 示例：
```rust
pub const DBFS_DIR_INODE_OPS: InodeOps = {
    let mut ops = InodeOps::empty();
    ops.create = dbfs_create;
    ops.mkdir = dbfs_mkdir;
    // ...
    ops
};
```

### 新 RVFS API (vfscore)
- 使用 trait：`VfsInode`, `VfsFile`, `VfsSuperBlock`, `VfsFsType`
- 面向对象设计
- 示例：
```rust
impl VfsInode for DbfsInode {
    fn create(&self, name: &str, ty: VfsNodeType, perm: VfsNodePerm, rdev: Option<u64>)
        -> VfsResult<Arc<dyn VfsInode>> {
        // 实现
    }
    // ...
}
```

## 新模块结构

```
src/rvfs2/
├── mod.rs          # 模块导出
├── superblock.rs   # DbfsSuperBlock 实现 (VfsSuperBlock trait)
├── inode.rs        # DbfsInode 实现 (VfsInode + VfsFile traits)
└── fstype.rs       # DbfsFsType 实现 (VfsFsType trait)
```

## 实现状态

✅ **已完成**：
- [x] 创建模块结构
- [x] DbfsSuperBlock - 实现 `VfsSuperBlock` trait
- [x] DbfsInode - 实现 `VfsInode` 和 `VfsFile` traits
- [x] DbfsFsType - 实现 `VfsFsType` trait
- [x] 更新 Cargo.toml 添加 vfscore 依赖

⚠️ **待完成**：
- [ ] 修复编译错误（dentry 集成）
- [ ] 实现 VfsDentry 集成
- [ ] 添加错误处理完善
- [ ] 集成测试
- [ ] 性能优化

## 核心实现

### 1. DbfsSuperBlock
管理整个文件系统的超级块，负责：
- 数据库实例管理
- Inode 缓存
- 文件系统统计 (`stat_fs`)
- 文件系统同步 (`sync_fs`)

### 2. DbfsInode
表示文件系统的 inode（文件或目录），实现：
- `VfsInode`: create, lookup, unlink, rename 等操作
- `VfsFile`: read_at, write_at, readdir 等操作
- 复用现有的 `dbfs_common_*` 函数

### 3. DbfsFsType
文件系统类型，负责：
- 挂载文件系统 (`mount`)
- 卸载文件系统 (`kill_sb`)
- 返回文件系统标志和名称

## 使用方法

### 编译（使用新的 rvfs2 特性）

```bash
cargo build --features rvfs2
```

### 基本用法（示例）

```rust
use dbfs2::rvfs2::DbfsFsType;
use std::sync::Arc;

// 创建文件系统类型
let fs_type = Arc::new(DbfsFsType::new("/path/to/dbfs.db".to_string()));

// 挂载文件系统
let root_dentry = fs_type.mount(0, "/", None, &[]).unwrap();

// 使用文件系统...
```

## 向后兼容性

- 旧代码（使用 `rvfs` feature）仍然可用
- 新代码（使用 `rvfs2` feature）使用 vfscore API
- 两者可以共存，但不会同时启用

## 参考资料

### 仓库链接

- **RVFS 仓库**: https://github.com/Godones/rvfs.git
  - [vfscore 接口定义](https://github.com/Godones/rvfs/tree/main/vfscore)
  - [ramfs 示例实现](https://github.com/Godones/rvfs/tree/main/ramfs)
  - [其他文件系统示例](https://github.com/Godones/rvfs/tree/main) (fat, ext4, devfs, etc.)

- **DBFS 仓库**: https://github.com/Godones/dbfs2.git
  - [RVFS2 适配代码](https://github.com/Godones/dbfs2/tree/main/src/rvfs2)
  - [核心业务逻辑](https://github.com/Godones/dbfs2/tree/main/src)

### 文档和示例

- [ramfs 完整实现](/home/ubuntu2204/Desktop/rvfs/ramfs/src/) - 新 RVFS API 的最佳参考
- [vfscore trait 定义](/home/ubuntu2204/Desktop/rvfs/vfscore/src/) - VFS 接口定义
- [RVFS2 适配进度](https://github.com/Godones/dbfs2/blob/main/RVFS2_ADAPTATION.md) - 本文档

## 开发者注意事项

1. **数据结构设计**：DbfsInode 使用 Mutex 保护可变状态
2. **缓存策略**：DbfsSuperBlock 维护 inode 缓存以提高性能
3. **错误处理**：将 `DbfsError` 转换为 `VfsError`
4. **时间戳**：当前使用占位符，需要集成实际时间源

## 下一步工作

1. **修复 dentry 集成**
   - 完善根 dentry 创建
   - 实现 dentry 操作

2. **完善错误处理**
   - 更好的错误转换
   - 错误传播

3. **添加测试**
   - 单元测试
   - 集成测试
   - 性能测试

4. **文档完善**
   - API 文档
   - 使用示例
   - 迁移指南
