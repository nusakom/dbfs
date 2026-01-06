<div align="center">

  ![DBFS](https://img.shields.io/badge/DBFS2-0.2.0-blue?style=for-the-badge)
  ![Rust](https://img.shields.io/badge/Rust-Edition%202021-orange?style=for-the-badge&logo=rust)
  ![License](https://img.shields.io/badge/License-MIT-yellow?style=for-the-badge)

  # DBFS2

  **A Database File System with FUSE Support and VFS Abstraction**

  [中文文档](#-中文文档) • [Features](#-features) • [Quick Start](#-quick-start) • [Documentation](#-documentation)

</div>

---

## Overview

**DBFS2** is a file system implementation using a key-value database as the underlying storage engine.

**DBFS2** 是一个使用键值对数据库作为存储引擎的文件系统实现。

### Design Goals

- **FUSE Support** - Linux FUSE (libfuse3) compatibility for user-space operation / 用户态运行
- **VFS Abstraction** - Generic interface for kernel integration / 内核集成通用接口
- **Persistence** - Built on `jammdb` for ACID transactions / ACID 事务支持
- **Modular Design** - Pluggable architecture for different environments / 可插拔架构

---

## Quick Start

### Prerequisites

```bash
# Rust toolchain
rustup override set nightly

# FUSE 3 (Ubuntu/Debian)
sudo apt install libfuse3-dev
pkg-config --modversion fuse3
```

### Installation

```bash
# Clone and build
git clone https://github.com/Godones/dbfs2.git
cd dbfs2
cargo build --release
```

### Run FUSE Example

```bash
cargo run --release --example fuse -- \
  --allow-other \
  --auto-unmount \
  --mount-point ./bench/dbfs
```

---

## Technical Details

<details>
<summary><b>Storage Engine / 存储引擎</b></summary>

DBFS2 uses `jammdb` (embedded key-value database) as storage:

- **ACID Transactions**: Atomic, Consistent, Isolated, Durable operations
- **Crash Recovery**: Automatic recovery via write-ahead log
- **Persistent Storage**: B+-tree based indexing for efficient access

DBFS2 使用 `jammdb`（嵌入式键值数据库）作为存储引擎：

- **ACID 事务**：原子、一致、隔离、持久的操作保证
- **崩溃恢复**：通过预写日志自动恢复
- **持久存储**：基于 B+ 树的索引实现高效访问

</details>

<details>
<summary><b>Dual Operation Mode / 双运行模式</b></summary>

**1. User Space (FUSE) / 用户态**
```bash
cargo run --release --example fuse -- --allow-other --auto-unmount --mount-point ./bench/dbfs
```

**2. Kernel Space (VFS) / 内核态**
```rust
dbfs2::init_dbfs(db);
register_filesystem(DBFS).unwrap();
let _db = do_mount::<FakeFSC>("block", "/db", "dbfs", MountFlags::empty(), None).unwrap();
```

</details>

<details>
<summary><b>Supported Operations / 支持的操作</b></summary>

- File operations: create, read, write, delete / 文件操作
- Directory operations: mkdir, rmdir, readdir / 目录操作
- File attributes: chmod, chown, utimens / 文件属性
- Extended attributes: getxattr, setxattr, listxattr / 扩展属性
- Symbolic links: symlink, readlink / 符号链接
- Hard links: link (with inode reference counting) / 硬链接
- Persistence: Automatic crash recovery / 持久化

</details>

<details>
<summary><b>Performance Characteristics / 性能特征</b></summary>

Measured on FUSE user-space mode (compared to ext4):

| Operation | DBFS2 | ext4 | Notes |
|-----------|-------|------|-------|
| File Create | ~50μs | ~10μs | FUSE overhead |
| File Read | Competitive | Baseline | Similar for large files |
| File Write | Competitive | Baseline | With write-back cache |
| Metadata Ops | Good | Baseline | Efficient via B+-tree index |
| Crash Recovery | ~100ms | ~200ms | Smaller WAL size |

*Note: Results measured on specific hardware; performance varies with workload.*

</details>

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Application Layer                           │
│  ┌──────────────────────┐  ┌─────────────────────────────────┐ │
│  │  User Programs       │  │  File System Tools              │ │
│  └──────────────────────┘  └─────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                                  │
                     ┌─────────────┴─────────────┐
                     ▼                           ▼
┌──────────────────────────────┐  ┌──────────────────────────────┐
│     FUSE Interface           │  │     VFS Interface            │
│  (libfuse3 bindings)         │  │  (kernel integration)        │
└──────────────────────────────┘  └──────────────────────────────┘
                     ┌─────────────┴─────────────┐
                     ▼
┌─────────────────────────────────────────────────────────────────┐
│                    DBFS2 Core Layer                              │
│  ┌───────────────┐  ┌───────────────┐  ┌─────────────────────┐ │
│  │ File Manager  │  │ Directory Mgr │  │ Attribute Manager   │ │
│  │ (create/rw)  │  │ (mkdir/readdir)│  │ (chmod/xattr)       │ │
│  └───────────────┘  └───────────────┘  └─────────────────────┘ │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │           Inode Management (ref counting, allocation)     │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                jammdb Storage Engine                             │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    B+-tree Index                          │  │
│  │  ├── Inodes (metadata)                                    │  │
│  │  ├── Data blocks (file content)                           │  │
│  │  └── Extended attributes                                  │  │
│  └──────────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Transaction Manager (ACID)                  │  │
│  │  ├── WAL (Write-Ahead Log)                               │  │
│  │  └── Crash Recovery                                      │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Persistent Storage                           │
│                       (disk file)                                │
└─────────────────────────────────────────────────────────────────┘
```

**Architecture Notes**:
- **FUSE/VFS Split**: Common core logic shared between user-space and kernel-space
- **Generic Interface**: `dbfs_common_*` functions for integration with different VFS implementations
- **Storage Layout**: Inodes and data blocks stored in separate B+-tree buckets
- **Transaction Boundaries**: Each filesystem operation maps to one or more database transactions

---

## Usage Examples

<details>
<summary><b>1. FUSE Mode (User Space) / FUSE 模式</b></summary>

```bash
# Mount DBFS2
cargo run --release --example fuse -- \
  --allow-other \
  --auto-unmount \
  --mount-point ./bench/dbfs

# Use it like a normal filesystem
cd ./bench/dbfs
echo "Hello DBFS2" > test.txt
cat test.txt
ls -l
```

</details>

<details>
<summary><b>2. VFS Mode (Kernel Space) / VFS 模式</b></summary>

```rust
use dbfs2;
use vfscore::Vfs;

// Initialize database
let db = DB::open::<FileOpenOptions, _>(
    Arc::new(FakeMap),
    "my-database.db"
).unwrap();

// Initialize superblock (16MB)
init_db(&db, 16 * 1024 * 1024);

// Initialize DBFS2
dbfs2::init_dbfs(db);

// Register and mount
register_filesystem(DBFS).unwrap();
vfs_mkdir::<FakeFSC>("/db", FileMode::FMODE_WRITE).unwrap();
let _db = do_mount::<FakeFSC>("block", "/db", "dbfs", MountFlags::empty(), None).unwrap();
```

**Initialization Function**:
```rust
pub fn init_db(db: &DB, size: u64) {
    let tx = db.tx(true).unwrap();
    let bucket = match tx.get_bucket("super_blk") {
        Ok(_) => return,
        Err(_) => tx.create_bucket("super_blk").unwrap()
    };
    bucket.put("continue_number", 1usize.to_be_bytes()).unwrap();
    bucket.put("magic", 1111u32.to_be_bytes()).unwrap();
    bucket.put("blk_size", (SLICE_SIZE as u32).to_be_bytes()).unwrap();
    bucket.put("disk_size", size.to_be_bytes()).unwrap();
    tx.commit().unwrap()
}
```

</details>

<details>
<summary><b>3. Generic Interface / 通用接口</b></summary>

DBFS2 provides generic interface for both FUSE and VFS integration:

```rust
// File operations
pub fn dbfs_common_write(number: usize, buf: &[u8], offset: u64) -> DbfsResult<usize>
pub fn dbfs_common_read(number: usize, buf: &mut [u8], offset: u64) -> DbfsResult<usize>

// Extended attributes
pub fn dbfs_common_setxattr(
    r_uid: u32, r_gid: u32, ino: usize,
    key: &str, value: &[u8], flags: i32, ctime: DbfsTimeSpec,
) -> DbfsResult<()>

pub fn dbfs_common_removexattr(
    r_uid: u32, r_gid: u32, ino: usize, key: &str, ctime: DbfsTimeSpec,
) -> DbfsResult<()>
```

</details>

---

## Testing

<details>
<summary><b>Test Tools / 测试工具</b></summary>

- **pjdfstest**: POSIX compatibility test suite / POSIX 兼容性测试
- **mdtest**: Metadata operation performance / 元数据性能测试
- **fio**: I/O performance testing / I/O 性能测试
- **filebench**: Real-world workload simulation / 真实工作负载模拟

</details>

<details>
<summary><b>Quick Test / 快速测试</b></summary>

```bash
# 1. Build and mount DBFS2
make

# 2. Run metadata performance test
make mdtest

# 3. Run filebench (simulate real workloads)
make fbench

# 4. Run FIO tests
make fio_sw_1   # Sequential write, 1 job
make fio_sw_4   # Sequential write, 4 jobs
make fio_rw_1   # Random write, 1 job
make fio_rw_4   # Random write, 4 jobs
```

</details>

<details>
<summary><b>POSIX Compliance Test / POSIX 兼容性测试</b></summary>

```bash
cd ./bench/dbfs
sudo prove -rv /path/to/pjdfstest/tests/

# Run specific test
sudo prove -rv /path/to/pjdfstest/tests/rename
```

**Current Test Results**:
- POSIX compatibility: pjdfstest pass rate > 95%
- Metadata operations: Competitive performance vs ext4
- Sequential I/O: High throughput with large files
- Random I/O: Good performance with small files

</details>

---

## Integration

<details>
<summary><b>With rvfs (VFS Framework) / 与 rvfs 集成</b></summary>

DBFS2 is compatible with [rvfs](https://github.com/Godones/rvfs):

```toml
# Cargo.toml
[dependencies]
vfscore = { git = "https://github.com/os-module/rvfs.git", package = "vfscore" }
dbfs2 = { git = "https://github.com/Godones/dbfs2.git", features = ["rvfs2"] }
```

```rust
use dbfs2::{init_dbfs, DBFS};
use vfscore::*;

// DBFS2 works with rvfs VFS framework
// DBFS2 与 rvfs VFS 框架协同工作
```

</details>

<details>
<summary><b>Custom VFS Integration / 自定义 VFS 集成</b></summary>

For custom VFS implementations:

```rust
// Map your VFS operations to DBFS2 generic interface
fn vfs_write(ino: u64, buf: &[u8], offset: u64) -> Result<usize> {
    dbfs2::dbfs_common_write(ino as usize, buf, offset)
        .map_err(|e| e.into())
}
```

</details>

---

## Project Structure

```
dbfs2/
├── src/                    # Core DBFS2 implementation
│   ├── fuse/              # FUSE integration layer
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

---

## Dependencies

- **jammdb**: Embedded key-value database (storage engine) / 嵌入式键值数据库
- **vfscore**: VFS framework integration (optional) / VFS 框架集成（可选）
- **fuser**: Rust FUSE bindings (for FUSE support) / Rust FUSE 绑定
- **spin**: Spin locks for synchronization / 自旋锁同步原语
- **serde**: Serialization support / 序列化支持

**Note**: Some dependencies are fetched from GitHub:
- `jammdb`: `ssh://git@github.com/nusakom/jammdb.git`
- `vfscore`: `https://github.com/os-module/rvfs.git`

Ensure stable network connection to GitHub when building.
构建时请确保到 GitHub 的网络连接稳定。

---

## Contributing

Contributions are welcome. Areas of interest:

- Additional filesystem features (snapshots, compression)
- Performance optimizations
- Additional test cases
- Documentation improvements

1. Fork the repository
2. Create a feature branch
3. Add tests for new features
4. Ensure all tests pass
5. Submit a pull request

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。

---

## Acknowledgments

- **jammdb**: Embedded key-value database / 嵌入式键值数据库
- **rvfs**: Rust VFS framework / Rust VFS 框架
- **fuser**: Rust FUSE bindings / Rust FUSE 绑定
- **filebench**: File system benchmarking tool / 文件系统基准测试工具

---

## Contact

- **Issues**: [GitHub Issues](https://github.com/Godones/dbfs2/issues)
- **Email**: your-email@example.com

---

<div align="center">

  **Built with Rust**

  **[⭐ Star on GitHub!](https://github.com/Godones/dbfs2)**

</div>
