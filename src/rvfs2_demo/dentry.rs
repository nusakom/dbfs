//! DBFS Dentry - Minimal demo implementation

use alloc::{collections::BTreeMap, string::String, string::ToString, sync::Arc};
use spin::Mutex;
use vfscore::{
    dentry::VfsDentry,
    inode::VfsInode,
    utils::VfsNodeType,
    VfsResult,
};

/// DBFS Dentry - minimal demo implementation
pub struct DbfsDentry {
    inner: Mutex<DbfsDentryInner>,
}

struct DbfsDentryInner {
    parent: Option<Arc<dyn VfsDentry>>,
    inode: Arc<dyn VfsInode>,
    name: String,
    children: BTreeMap<String, Arc<dyn VfsDentry>>,
}

impl DbfsDentry {
    /// Create root dentry
    pub fn root(inode: Arc<dyn VfsInode>) -> Self {
        Self {
            inner: Mutex::new(DbfsDentryInner {
                parent: None,
                inode,
                name: "/".to_string(),
                children: BTreeMap::new(),
            }),
        }
    }
}

impl VfsDentry for DbfsDentry {
    fn name(&self) -> String {
        self.inner.lock().name.clone()
    }

    fn to_mount_point(
        self: Arc<Self>,
        _sub_fs_root: Arc<dyn VfsDentry>,
        _mount_flag: u32,
    ) -> VfsResult<()> {
        Err(vfscore::error::VfsError::NoSys)
    }

    fn inode(&self) -> VfsResult<Arc<dyn VfsInode>> {
        Ok(self.inner.lock().inode.clone())
    }

    fn mount_point(&self) -> Option<vfscore::fstype::VfsMountPoint> {
        None
    }

    fn clear_mount_point(&self) {
        // Nothing to clear
    }

    fn find(&self, path: &str) -> Option<Arc<dyn VfsDentry>> {
        let inner = self.inner.lock();
        let inode_type = inner.inode.inode_type();
        if inode_type == VfsNodeType::Dir {
            inner.children.get(path).cloned()
        } else {
            None
        }
    }

    fn insert(
        self: Arc<Self>,
        name: &str,
        child: Arc<dyn VfsInode>,
    ) -> VfsResult<Arc<dyn VfsDentry>> {
        let dentry = Arc::new(Self {
            inner: Mutex::new(DbfsDentryInner {
                parent: Some(self.clone() as Arc<dyn VfsDentry>),
                inode: child,
                name: name.to_string(),
                children: BTreeMap::new(),
            }),
        });

        let mut inner = self.inner.lock();
        if inner.children.contains_key(name) {
            return Err(vfscore::error::VfsError::EExist);
        }
        inner.children.insert(name.to_string(), dentry.clone());
        Ok(dentry)
    }

    fn remove(&self, name: &str) -> Option<Arc<dyn VfsDentry>> {
        let mut inner = self.inner.lock();
        inner.children.remove(name)
    }

    fn parent(&self) -> Option<Arc<dyn VfsDentry>> {
        self.inner.lock().parent.as_ref().cloned()
    }

    fn set_parent(&self, parent: &Arc<dyn VfsDentry>) {
        self.inner.lock().parent = Some(parent.clone());
    }
}
