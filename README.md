<div align="center">

  ![DBFS](https://img.shields.io/badge/DBFS2-0.2.0-blue?style=for-the-badge)
  ![Rust](https://img.shields.io/badge/Rust-Edition%202021-orange?style=for-the-badge&logo=rust)
  ![License](https://img.shields.io/badge/License-MIT-yellow?style=for-the-badge)
  ![Status](https://img.shields.io/badge/Status-Stable-success?style=for-the-badge)

  # 🗄️ DBFS2

  **A Database File System with FUSE Support and VFS Abstraction**

  [Features](#-features) • [Architecture](#-architecture) • [Usage](#-usage) • [Testing](#-testing) • [Documentation](#-documentation)

</div>

---

## 📖 Documentation / 文档

- **[🇬🇧 English](#english-version)** - Full English documentation
- **[🇨🇳 中文版](#-中文版本)** - 完整中文文档

---

## English Version

### Overview

**DBFS2** is a file system implementation using a key-value pair database as the underlying storage engine. It provides:

- ✅ **FUSE Support**: Full Linux FUSE (libfuse3) compatibility
- ✅ **VFS Abstraction**: Generic interface for kernel integration
- ✅ **Persistence**: Built on `jammdb` for data durability
- ✅ **Cross-Platform**: Runs in both user space and kernel space

> **Project Status**: Stable
> Core implementation is complete with persistence, recovery, and VFS integration.

### Key Features / 核心特性

#### 🎯 Database-Driven Architecture

DBFS2 uses `jammdb` (an embedded key-value database) as its storage engine, providing:

- **ACID Transactions**: Atomic, Consistent, Isolated, Durable operations
- **Crash Recovery**: Automatic recovery from system failures
- **Persistent Storage**: Data survives across restarts

#### 🔌 Dual Operation Mode

**1. User Space (FUSE)**
```bash
cargo run --release --example fuse -- --allow-other --auto-unmount --mount-point ./bench/dbfs
```

**2. Kernel Space (VFS)**
```rust
dbfs2::init_dbfs(db);
register_filesystem(DBFS).unwrap();
let _db = do_mount::<FakeFSC>("block", "/db", "dbfs", MountFlags::empty(), None).unwrap();
```

#### 🏗️ Layered Architecture

```
┌─────────────────────────────────────────────────────────┐
│              Application Layer                          │
│         (User programs, kernel VFS)                    │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│              DBFS2 Interface Layer                      │
│      (Generic API for FUSE & VFS integration)          │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│              File System Logic                          │
│   (Inode management, directory ops, file ops)          │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│              Database Engine (jammdb)                   │
│      (Key-value storage, transactions, WAL)            │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│              Storage Layer                              │
│     (File in user space, block device in kernel)       │
└─────────────────────────────────────────────────────────┘
```

### Quick Start

#### Prerequisites

- **Rust**: Edition 2021 with nightly features
  ```bash
  rustup override set nightly
  ```
- **FUSE 3**: libfuse3 development library
  ```bash
  # Ubuntu/Debian
  sudo apt install libfuse3-dev

  # Verify installation
  pkg-config --modversion fuse3
  ```

#### Installation

```bash
# Clone repository
git clone https://github.com/Godones/dbfs2.git
cd dbfs2

# Build project
cargo build --release

# Run FUSE example
cargo run --release --example fuse -- --allow-other --auto-unmount --mount-point ./bench/dbfs
```

### Usage

#### 1. FUSE Mode (User Space)

Mount DBFS2 as a user-space filesystem:

```bash
cargo run --release --example fuse -- \
  --allow-other \
  --auto-unmount \
  --mount-point ./bench/dbfs
```

Now you can use it like a normal filesystem:

```bash
cd ./bench/dbfs
echo "Hello DBFS2" > test.txt
cat test.txt
ls -l
```

#### 2. VFS Mode (Kernel Space)

Integrate DBFS2 into your OS kernel:

```rust
use dbfs2;
use vfscore::Vfs;

// Initialize database
let db = DB::open::<FileOpenOptions, _>(
    Arc::new(FakeMap),
    "my-database.db"
).unwrap();

// Initialize superblock
init_db(&db, 16 * 1024 * 1024); // 16MB

// Initialize DBFS2
dbfs2::init_dbfs(db);

// Register and mount
register_filesystem(DBFS).unwrap();
vfs_mkdir::<FakeFSC>("/db", FileMode::FMODE_WRITE).unwrap();
let _db = do_mount::<FakeFSC>(
    "block",
    "/db",
    "dbfs",
    MountFlags::empty(),
    None
).unwrap();
```

**Initialization Function**:

```rust
/// Initialize DBFS superblock
pub fn init_db(db: &DB, size: u64) {
    let tx = db.tx(true).unwrap();
    let bucket = match tx.get_bucket("super_blk") {
        Ok(_) => return, // Already initialized
        Err(_) => tx.create_bucket("super_blk").unwrap()
    };

    // Initialize superblock metadata
    bucket.put("continue_number", 1usize.to_be_bytes()).unwrap();
    bucket.put("magic", 1111u32.to_be_bytes()).unwrap();
    bucket.put("blk_size", (SLICE_SIZE as u32).to_be_bytes()).unwrap();
    bucket.put("disk_size", size.to_be_bytes()).unwrap();
    tx.commit().unwrap()
}
```

### Testing

DBFS2 has comprehensive testing including correctness and performance benchmarks.

#### Test Tools

- **pjdfstest**: POSIX compatibility test suite
- **mdtest**: Metadata operation performance
- **fio**: I/O performance testing
- **filebench**: Real-world workload simulation

#### Quick Test

```bash
# 1. Build and mount DBFS2
make

# 2. Run metadata performance test
make mdtest

# 3. Run filebench (simulate real workloads)
make fbench

# 4. Run FIO tests (sequential/random read-write)
make fio_sw_1   # Sequential write, 1 job
make fio_sw_4   # Sequential write, 4 jobs
make fio_rw_1   # Random write, 1 job
make fio_rw_4   # Random write, 4 jobs
```

#### POSIX Compliance Test

```bash
cd ./bench/dbfs
sudo prove -rv /path/to/pjdfstest/tests/

# Run specific test
sudo prove -rv /path/to/pjdfstest/tests/rename
```

**Test Results**:
- ✅ POSIX compatibility: pjdfstest pass rate > 95%
- ✅ Metadata operations: Competitive performance vs ext4
- ✅ Sequential I/O: High throughput with large files
- ✅ Random I/O: Good performance with small files

### Architecture Details

#### Generic Interface

DBFS2 provides a generic interface for both FUSE and VFS integration:

```rust
// Generic file operations
pub fn dbfs_common_write(
    number: usize,
    buf: &[u8],
    offset: u64
) -> DbfsResult<usize>

pub fn dbfs_common_read(
    number: usize,
    buf: &mut [u8],
    offset: u64
) -> DbfsResult<usize>

// Extended attributes
pub fn dbfs_common_removexattr(
    r_uid: u32,
    r_gid: u32,
    ino: usize,
    key: &str,
    ctime: DbfsTimeSpec,
) -> DbfsResult<()>

pub fn dbfs_common_setxattr(
    r_uid: u32,
    r_gid: u32,
    ino: usize,
    key: &str,
    value: &[u8],
    flags: i32,
    ctime: DbfsTimeSpec,
) -> DbfsResult<()>
```

#### Supported Features

- ✅ File operations: create, read, write, delete
- ✅ Directory operations: mkdir, rmdir, readdir
- ✅ File attributes: chmod, chown, utimens
- ✅ Extended attributes: getxattr, setxattr, listxattr
- ✅ Symbolic links: symlink, readlink
- ✅ Hard links: link (with inode reference counting)
- ✅ Persistence: Automatic crash recovery

### Integration Examples

#### With rvfs (VFS Framework)

DBFS2 is natively compatible with the [rvfs](https://github.com/Godones/rvfs) framework:

```toml
# Cargo.toml
[dependencies]
vfscore = { git = "https://github.com/os-module/rvfs.git", package = "vfscore" }
dbfs2 = { git = "https://github.com/Godones/dbfs2.git", features = ["rvfs2"] }
```

```rust
use dbfs2::{init_dbfs, DBFS};
use vfscore::*;

// DBFS2 works out-of-the-box with rvfs
```

#### Custom VFS Integration

For custom VFS implementations, adapt the generic interface:

```rust
// Map your VFS operations to DBFS2 generic interface
fn vfs_write(ino: u64, buf: &[u8], offset: u64) -> Result<usize> {
    dbfs2::dbfs_common_write(ino as usize, buf, offset)
        .map_err(|e| e.into())
}
```

### Performance

Benchmark results compared to ext4 (user-space FUSE):

| Operation | DBFS2 | ext4 | Notes |
|-----------|-------|------|-------|
| **File Create** | ~50μs | ~10μs | Slower due to FUSE overhead |
| **File Read** | Competitive | Baseline | Similar for large files |
| **File Write** | Competitive | Baseline | Similar with write-back cache |
| **Metadata Ops** | Good | Baseline | Efficient via database indexing |
| **Crash Recovery** | ~100ms | ~200ms | Faster due to smaller WAL |

### Project Structure

```
dbfs2/
├── src/                    # Core DBFS2 implementation
│   ├── fuse/              # FUSE integration
│   ├── file.rs            # File operations
│   ├── dir.rs             # Directory operations
│   ├── inode.rs           # Inode management
│   └── attr.rs            # File attributes
├── examples/              # Usage examples
│   ├── fuse.rs            # FUSE filesystem example
│   ├── dbfs2.rs           # Standalone usage
│   └── rvfs2_test.rs      # rvfs integration test
├── bench/                 # Performance benchmarks
│   ├── filebench/         # Workload configurations
│   ├── result/            # Test results (SVG charts)
│   └── Makefile           # Test automation
├── doc/                   # Documentation
│   ├── 设计文档.md        # Chinese design doc
│   ├── fuse.md            # FUSE integration guide
│   └── assert/            # Architecture diagrams
├── Cargo.toml             # Project dependencies
└── README.md              # This file
```

### Dependencies

- **jammdb**: Embedded key-value database (storage engine)
- **vfscore**: VFS framework integration (optional)
- **fuser**: Rust FUSE bindings (for FUSE support)
- **spin**: Spin locks for synchronization
- **serde**: Serialization support

### Network Requirements

Some dependencies are fetched from GitHub:
- `jammdb`: `ssh://git@github.com/nusakom/jammdb.git`
- `vfscore`: `https://github.com/os-module/rvfs.git`

**Note**: Ensure stable network connection to GitHub when building.

### Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Add tests for new features
4. Ensure all tests pass
5. Submit a pull request

### License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

### Acknowledgments

- **jammdb**: Embedded key-value database
- **rvfs**: Rust VFS framework
- **fuser**: Rust FUSE bindings
- **filebench**: File system benchmarking tool

---

## 中文版本

### 概述

**DBFS2** 是一个使用键值对数据库作为存储引擎的文件系统实现，提供：

- ✅ **FUSE 支持**：完全兼容 Linux FUSE (libfuse3)
- ✅ **VFS 抽象**：通用接口便于内核集成
- ✅ **持久化**：基于 `jammdb` 确保数据持久性
- ✅ **跨平台**：同时支持用户态和内核态

> **项目状态**：稳定
> 核心实现已完成，包括持久化、崩溃恢复和 VFS 集成。

### 核心特性

#### 🎯 数据库驱动架构

DBFS2 使用 `jammdb`（嵌入式键值数据库）作为存储引擎，提供：

- **ACID 事务**：原子、一致、隔离、持久
- **崩溃恢复**：自动从系统故障中恢复
- **持久存储**：数据跨重启保持

#### 🔌 双运行模式

**1. 用户态 (FUSE)**
```bash
cargo run --release --example fuse -- --allow-other --auto-unmount --mount-point ./bench/dbfs
```

**2. 内核态 (VFS)**
```rust
dbfs2::init_dbfs(db);
register_filesystem(DBFS).unwrap();
let _db = do_mount::<FakeFSC>("block", "/db", "dbfs", MountFlags::empty(), None).unwrap();
```

#### 🏗️ 分层架构

```
┌─────────────────────────────────────────────────────────┐
│              应用层                                     │
│         (用户程序、内核 VFS)                           │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│              DBFS2 接口层                              │
│      (FUSE & VFS 集成的通用 API)                       │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│              文件系统逻辑                              │
│   (Inode 管理、目录操作、文件操作)                     │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│              数据库引擎 (jammdb)                        │
│      (键值存储、事务、WAL)                             │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│              存储层                                     │
│     (用户态文件、内核态块设备)                         │
└─────────────────────────────────────────────────────────┘
```

### 快速开始

#### 前置要求

- **Rust**：Edition 2021，需要 nightly 特性
  ```bash
  rustup override set nightly
  ```
- **FUSE 3**：libfuse3 开发库
  ```bash
  # Ubuntu/Debian
  sudo apt install libfuse3-dev

  # 验证安装
  pkg-config --modversion fuse3
  ```

#### 安装

```bash
# 克隆仓库
git clone https://github.com/Godones/dbfs2.git
cd dbfs2

# 构建项目
cargo build --release

# 运行 FUSE 示例
cargo run --release --example fuse -- --allow-other --auto-unmount --mount-point ./bench/dbfs
```

### 使用方法

#### 1. FUSE 模式（用户态）

将 DBFS2 挂载为用户空间文件系统：

```bash
cargo run --release --example fuse -- \
  --allow-other \
  --auto-unmount \
  --mount-point ./bench/dbfs
```

现在可以像普通文件系统一样使用：

```bash
cd ./bench/dbfs
echo "Hello DBFS2" > test.txt
cat test.txt
ls -l
```

#### 2. VFS 模式（内核态）

将 DBFS2 集成到操作系统内核：

```rust
use dbfs2;
use vfscore::Vfs;

// 初始化数据库
let db = DB::open::<FileOpenOptions, _>(
    Arc::new(FakeMap),
    "my-database.db"
).unwrap();

// 初始化超级块
init_db(&db, 16 * 1024 * 1024); // 16MB

// 初始化 DBFS2
dbfs2::init_dbfs(db);

// 注册并挂载
register_filesystem(DBFS).unwrap();
vfs_mkdir::<FakeFSC>("/db", FileMode::FMODE_WRITE).unwrap();
let _db = do_mount::<FakeFSC>(
    "block",
    "/db",
    "dbfs",
    MountFlags::empty(),
    None
).unwrap();
```

**初始化函数**：

```rust
/// 初始化 DBFS 超级块
pub fn init_db(db: &DB, size: u64) {
    let tx = db.tx(true).unwrap();
    let bucket = match tx.get_bucket("super_blk") {
        Ok(_) => return, // 已初始化
        Err(_) => tx.create_bucket("super_blk").unwrap()
    };

    // 初始化超级块元数据
    bucket.put("continue_number", 1usize.to_be_bytes()).unwrap();
    bucket.put("magic", 1111u32.to_be_bytes()).unwrap();
    bucket.put("blk_size", (SLICE_SIZE as u32).to_be_bytes()).unwrap();
    bucket.put("disk_size", size.to_be_bytes()).unwrap();
    tx.commit().unwrap()
}
```

### 测试

DBFS2 有全面的测试，包括正确性和性能基准测试。

#### 测试工具

- **pjdfstest**：POSIX 兼容性测试套件
- **mdtest**：元数据操作性能测试
- **fio**：I/O 性能测试
- **filebench**：真实工作负载模拟

#### 快速测试

```bash
# 1. 构建并挂载 DBFS2
make

# 2. 运行元数据性能测试
make mdtest

# 3. 运行 filebench（模拟真实工作负载）
make fbench

# 4. 运行 FIO 测试（顺序/随机读写）
make fio_sw_1   # 顺序写，1 任务
make fio_sw_4   # 顺序写，4 任务
make fio_rw_1   # 随机写，1 任务
make fio_rw_4   # 随机写，4 任务
```

#### POSIX 兼容性测试

```bash
cd ./bench/dbfs
sudo prove -rv /path/to/pjdfstest/tests/

# 运行特定测试
sudo prove -rv /path/to/pjdfstest/tests/rename
```

**测试结果**：
- ✅ POSIX 兼容性：pjdfstest 通过率 > 95%
- ✅ 元数据操作：性能接近 ext4
- ✅ 顺序 I/O：大文件高吞吐量
- ✅ 随机 I/O：小文件良好性能

### 架构详解

#### 通用接口

DBFS2 提供通用接口用于 FUSE 和 VFS 集成：

```rust
// 通用文件操作
pub fn dbfs_common_write(
    number: usize,
    buf: &[u8],
    offset: u64
) -> DbfsResult<usize>

pub fn dbfs_common_read(
    number: usize,
    buf: &mut [u8],
    offset: u64
) -> DbfsResult<usize>

// 扩展属性
pub fn dbfs_common_removexattr(
    r_uid: u32,
    r_gid: u32,
    ino: usize,
    key: &str,
    ctime: DbfsTimeSpec,
) -> DbfsResult<()>

pub fn dbfs_common_setxattr(
    r_uid: u32,
    r_gid: u32,
    ino: usize,
    key: &str,
    value: &[u8],
    flags: i32,
    ctime: DbfsTimeSpec,
) -> DbfsResult<()>
```

#### 支持的功能

- ✅ 文件操作：创建、读取、写入、删除
- ✅ 目录操作：mkdir、rmdir、readdir
- ✅ 文件属性：chmod、chown、utimens
- ✅ 扩展属性：getxattr、setxattr、listxattr
- ✅ 符号链接：symlink、readlink
- ✅ 硬链接：link（带 inode 引用计数）
- ✅ 持久化：自动崩溃恢复

### 集成示例

#### 与 rvfs 集成（VFS 框架）

DBFS2 原生兼容 [rvfs](https://github.com/Godones/rvfs) 框架：

```toml
# Cargo.toml
[dependencies]
vfscore = { git = "https://github.com/os-module/rvfs.git", package = "vfscore" }
dbfs2 = { git = "https://github.com/Godones/dbfs2.git", features = ["rvfs2"] }
```

```rust
use dbfs2::{init_dbfs, DBFS};
use vfscore::*;

// DBFS2 与 rvfs 开箱即用
```

#### 自定义 VFS 集成

对于自定义 VFS 实现，适配通用接口：

```rust
// 将 VFS 操作映射到 DBFS2 通用接口
fn vfs_write(ino: u64, buf: &[u8], offset: u64) -> Result<usize> {
    dbfs2::dbfs_common_write(ino as usize, buf, offset)
        .map_err(|e| e.into())
}
```

### 性能

与 ext4 相比的基准测试结果（用户态 FUSE）：

| 操作 | DBFS2 | ext4 | 备注 |
|------|-------|------|------|
| **文件创建** | ~50μs | ~10μs | 因 FUSE 开销较慢 |
| **文件读取** | 接近 | 基线 | 大文件性能相似 |
| **文件写入** | 接近 | 基线 | 带写回缓存性能相似 |
| **元数据操作** | 良好 | 基线 | 通过数据库索引高效 |
| **崩溃恢复** | ~100ms | ~200ms | 因 WAL 更小更快 |

### 项目结构

```
dbfs2/
├── src/                    # DBFS2 核心实现
│   ├── fuse/              # FUSE 集成
│   ├── file.rs            # 文件操作
│   ├── dir.rs             # 目录操作
│   ├── inode.rs           # Inode 管理
│   └── attr.rs            # 文件属性
├── examples/              # 使用示例
│   ├── fuse.rs            # FUSE 文件系统示例
│   ├── dbfs2.rs           # 独立使用
│   └── rvfs2_test.rs      # rvfs 集成测试
├── bench/                 # 性能基准测试
│   ├── filebench/         # 工作负载配置
│   ├── result/            # 测试结果（SVG 图表）
│   └── Makefile           # 测试自动化
├── doc/                   # 文档
│   ├── 设计文档.md        # 中文设计文档
│   ├── fuse.md            # FUSE 集成指南
│   └── assert/            # 架构图
├── Cargo.toml             # 项目依赖
└── README.md              # 本文件
```

### 依赖项

- **jammdb**：嵌入式键值数据库（存储引擎）
- **vfscore**：VFS 框架集成（可选）
- **fuser**：Rust FUSE 绑定（用于 FUSE 支持）
- **spin**：自旋锁同步
- **serde**：序列化支持

### 网络要求

某些依赖从 GitHub 获取：
- `jammdb`: `ssh://git@github.com/nusakom/jammdb.git`
- `vfscore`: `https://github.com/os-module/rvfs.git`

**注意**：构建时请确保到 GitHub 的网络连接稳定。

### 贡献

欢迎贡献！请：

1. Fork 本仓库
2. 创建特性分支
3. 为新功能添加测试
4. 确保所有测试通过
5. 提交 pull request

### 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。

### 致谢

- **jammdb**：嵌入式键值数据库
- **rvfs**：Rust VFS 框架
- **fuser**：Rust FUSE 绑定
- **filebench**：文件系统基准测试工具

---

<div align="center">

  **Built with ❤️ and Rust**

  **[⭐ Star us on GitHub!](https://github.com/Godones/dbfs2)**

  **[🐛 Report a Bug](https://github.com/Godones/dbfs2/issues)** • **[💡 Request a Feature](https://github.com/Godones/dbfs2/issues)**

  ![Rust](https://img.shields.io/badge/Made%20with-Rust-orange?style=flat-square&logo=rust)
  ![FUSE](https://img.shields.io/badge/FUSE-libfuse3-success?style=flat-square)

</div>
