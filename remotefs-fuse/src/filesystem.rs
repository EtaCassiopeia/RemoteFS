use async_trait::async_trait;
use bytes::Bytes;
use fuse3::raw::prelude::*;
use fuse3::{Errno, Result, MountOptions, Session};
use remotefs_client::RemoteFsClient;
use remotefs_common::{
    protocol::{FileType, FileMetadata, DirEntry},
    error::{RemoteFsError, Result as RemoteFsResult},
};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Default file and directory permissions
const DEFAULT_FILE_MODE: u32 = 0o644;
const DEFAULT_DIR_MODE: u32 = 0o755;

/// TTL for filesystem metadata (in seconds)
const ATTR_TTL: Duration = Duration::from_secs(10);
const ENTRY_TTL: Duration = Duration::from_secs(10);

/// RemoteFS FUSE filesystem implementation
pub struct RemoteFsFilesystem {
    client: Arc<RemoteFsClient>,
    mount_point: PathBuf,
    remote_root: PathBuf,
    uid: u32,
    gid: u32,
    /// Cache for inode to path mapping
    inode_to_path: Arc<RwLock<HashMap<u64, PathBuf>>>,
    /// Cache for path to inode mapping
    path_to_inode: Arc<RwLock<HashMap<PathBuf, u64>>>,
    /// Next available inode number
    next_inode: Arc<RwLock<u64>>,
    /// Cache for file attributes
    attr_cache: Arc<RwLock<HashMap<u64, (FileAttr, SystemTime)>>>,
}

impl RemoteFsFilesystem {
    /// Create a new RemoteFS filesystem
    pub fn new(
        client: Arc<RemoteFsClient>,
        mount_point: PathBuf,
        remote_root: PathBuf,
    ) -> Self {
        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };

        let mut inode_to_path = HashMap::new();
        let mut path_to_inode = HashMap::new();
        
        // Root directory gets inode 1
        inode_to_path.insert(1, PathBuf::from("/"));
        path_to_inode.insert(PathBuf::from("/"), 1);

        Self {
            client,
            mount_point,
            remote_root,
            uid,
            gid,
            inode_to_path: Arc::new(RwLock::new(inode_to_path)),
            path_to_inode: Arc::new(RwLock::new(path_to_inode)),
            next_inode: Arc::new(RwLock::new(2)),
            attr_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create an inode for a path
    async fn get_or_create_inode(&self, path: &Path) -> u64 {
        // Check if we already have an inode for this path
        {
            let path_to_inode = self.path_to_inode.read().await;
            if let Some(&inode) = path_to_inode.get(path) {
                return inode;
            }
        }

        // Create a new inode
        let mut next_inode = self.next_inode.write().await;
        let inode = *next_inode;
        *next_inode += 1;

        // Store the mapping
        {
            let mut inode_to_path = self.inode_to_path.write().await;
            let mut path_to_inode = self.path_to_inode.write().await;
            
            inode_to_path.insert(inode, path.to_path_buf());
            path_to_inode.insert(path.to_path_buf(), inode);
        }

        inode
    }

    /// Get the path for an inode
    async fn get_path_for_inode(&self, inode: u64) -> Option<PathBuf> {
        let inode_to_path = self.inode_to_path.read().await;
        inode_to_path.get(&inode).cloned()
    }

    /// Convert a mount path to a remote path
    fn mount_to_remote_path(&self, mount_path: &Path) -> PathBuf {
        if mount_path == Path::new("/") {
            self.remote_root.clone()
        } else {
            self.remote_root.join(mount_path.strip_prefix("/").unwrap_or(mount_path))
        }
    }

    /// Convert RemoteFS metadata to FUSE file attributes
    fn metadata_to_attr(&self, metadata: &FileMetadata, inode: u64) -> FileAttr {
        let file_type = match metadata.file_type {
            FileType::RegularFile => FileType3::RegularFile,
            FileType::Directory => FileType3::Directory,
            FileType::Symlink => FileType3::Symlink,
            FileType::BlockDevice => FileType3::BlockDevice,
            FileType::CharacterDevice => FileType3::CharDevice,
            FileType::Fifo => FileType3::NamedPipe,
            FileType::Socket => FileType3::Socket,
        };

        let mode = match metadata.file_type {
            FileType::Directory => DEFAULT_DIR_MODE,
            _ => DEFAULT_FILE_MODE,
        };

        FileAttr {
            ino: inode,
            size: metadata.size,
            blocks: (metadata.size + 511) / 512, // 512-byte blocks
            atime: UNIX_EPOCH + Duration::from_secs(metadata.accessed),
            mtime: UNIX_EPOCH + Duration::from_secs(metadata.modified),
            ctime: UNIX_EPOCH + Duration::from_secs(metadata.created),
            kind: file_type,
            perm: (mode & 0o777) as u16,
            nlink: 1,
            uid: self.uid,
            gid: self.gid,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        }
    }

    /// Get cached attributes or fetch from remote
    async fn get_attr(&self, inode: u64) -> Result<FileAttr> {
        // Check cache first
        {
            let cache = self.attr_cache.read().await;
            if let Some((attr, cached_at)) = cache.get(&inode) {
                if cached_at.elapsed().unwrap_or(Duration::from_secs(u64::MAX)) < ATTR_TTL {
                    return Ok(*attr);
                }
            }
        }

        // Get path for inode
        let path = self.get_path_for_inode(inode).await
            .ok_or(Errno::from(libc::ENOENT))?;

        let remote_path = self.mount_to_remote_path(&path);

        // Fetch metadata from remote
        match self.client.get_metadata(&remote_path).await {
            Ok(metadata) => {
                let attr = self.metadata_to_attr(&metadata, inode);
                
                // Cache the result
                let mut cache = self.attr_cache.write().await;
                cache.insert(inode, (attr, SystemTime::now()));
                
                Ok(attr)
            }
            Err(RemoteFsError::NotFound(_)) => Err(Errno::from(libc::ENOENT)),
            Err(RemoteFsError::PermissionDenied(_)) => Err(Errno::from(libc::EACCES)),
            Err(e) => {
                error!("Failed to get metadata for {}: {}", remote_path.display(), e);
                Err(Errno::from(libc::EIO))
            }
        }
    }

    /// Invalidate cache for an inode
    async fn invalidate_attr_cache(&self, inode: u64) {
        let mut cache = self.attr_cache.write().await;
        cache.remove(&inode);
    }
}

#[async_trait]
impl Filesystem for RemoteFsFilesystem {
    type DirEntryStream = Vec<Result<DirectoryEntry>>;

    async fn init(&self, _req: Request) -> Result<()> {
        info!("Initializing RemoteFS FUSE filesystem");
        info!("Mount point: {}", self.mount_point.display());
        info!("Remote root: {}", self.remote_root.display());
        
        // Test connection to remote
        match self.client.get_metadata(&self.remote_root).await {
            Ok(_) => {
                info!("Successfully connected to remote filesystem");
                Ok(())
            }
            Err(e) => {
                error!("Failed to connect to remote filesystem: {}", e);
                Err(Errno::from(libc::ECONNREFUSED))
            }
        }
    }

    async fn destroy(&self) -> Result<()> {
        info!("Destroying RemoteFS FUSE filesystem");
        Ok(())
    }

    async fn getattr(&self, _req: Request, inode: u64) -> Result<(Duration, FileAttr)> {
        debug!("getattr: inode={}", inode);
        
        let attr = self.get_attr(inode).await?;
        Ok((ATTR_TTL, attr))
    }

    async fn lookup(&self, _req: Request, parent: u64, name: &OsStr) -> Result<(Duration, FileAttr)> {
        debug!("lookup: parent={}, name={:?}", parent, name);
        
        let parent_path = self.get_path_for_inode(parent).await
            .ok_or(Errno::from(libc::ENOENT))?;
        
        let child_path = parent_path.join(name);
        let remote_path = self.mount_to_remote_path(&child_path);
        
        // Try to get metadata for the child
        match self.client.get_metadata(&remote_path).await {
            Ok(metadata) => {
                let inode = self.get_or_create_inode(&child_path).await;
                let attr = self.metadata_to_attr(&metadata, inode);
                
                // Cache the result
                let mut cache = self.attr_cache.write().await;
                cache.insert(inode, (attr, SystemTime::now()));
                
                Ok((ENTRY_TTL, attr))
            }
            Err(RemoteFsError::NotFound(_)) => Err(Errno::from(libc::ENOENT)),
            Err(RemoteFsError::PermissionDenied(_)) => Err(Errno::from(libc::EACCES)),
            Err(e) => {
                error!("Failed to lookup {}: {}", remote_path.display(), e);
                Err(Errno::from(libc::EIO))
            }
        }
    }

    async fn readdir(
        &self,
        _req: Request,
        inode: u64,
        _fh: u64,
        offset: i64,
    ) -> Result<Self::DirEntryStream> {
        debug!("readdir: inode={}, offset={}", inode, offset);
        
        let path = self.get_path_for_inode(inode).await
            .ok_or(Errno::from(libc::ENOENT))?;
        
        let remote_path = self.mount_to_remote_path(&path);
        
        match self.client.list_directory(&remote_path).await {
            Ok(entries) => {
                let mut result = Vec::new();
                
                // Add . and .. entries if offset allows
                if offset <= 0 {
                    result.push(Ok(DirectoryEntry {
                        inode,
                        index: 1,
                        kind: FileType3::Directory,
                        name: OsString::from("."),
                    }));
                }
                
                if offset <= 1 {
                    // For root directory, .. points to itself
                    let parent_inode = if inode == 1 { 1 } else {
                        // Get parent path
                        let parent_path = path.parent().unwrap_or(Path::new("/"));
                        self.get_or_create_inode(parent_path).await
                    };
                    
                    result.push(Ok(DirectoryEntry {
                        inode: parent_inode,
                        index: 2,
                        kind: FileType3::Directory,
                        name: OsString::from(".."),
                    }));
                }
                
                // Add actual directory entries
                for (i, entry) in entries.iter().enumerate() {
                    let index = (i + 3) as i64; // Start after . and ..
                    if index <= offset {
                        continue;
                    }
                    
                    let child_path = path.join(&entry.name);
                    let child_inode = self.get_or_create_inode(&child_path).await;
                    
                    let kind = match entry.file_type {
                        FileType::Directory => FileType3::Directory,
                        FileType::RegularFile => FileType3::RegularFile,
                        FileType::Symlink => FileType3::Symlink,
                        FileType::BlockDevice => FileType3::BlockDevice,
                        FileType::CharacterDevice => FileType3::CharDevice,
                        FileType::Fifo => FileType3::NamedPipe,
                        FileType::Socket => FileType3::Socket,
                    };
                    
                    result.push(Ok(DirectoryEntry {
                        inode: child_inode,
                        index,
                        kind,
                        name: OsString::from(&entry.name),
                    }));
                }
                
                Ok(result)
            }
            Err(RemoteFsError::NotFound(_)) => Err(Errno::from(libc::ENOENT)),
            Err(RemoteFsError::PermissionDenied(_)) => Err(Errno::from(libc::EACCES)),
            Err(e) => {
                error!("Failed to read directory {}: {}", remote_path.display(), e);
                Err(Errno::from(libc::EIO))
            }
        }
    }

    async fn open(&self, _req: Request, inode: u64, flags: u32) -> Result<(Option<u64>, u32)> {
        debug!("open: inode={}, flags={:o}", inode, flags);
        
        let path = self.get_path_for_inode(inode).await
            .ok_or(Errno::from(libc::ENOENT))?;
        
        let remote_path = self.mount_to_remote_path(&path);
        
        // Check if file exists and is accessible
        match self.client.get_metadata(&remote_path).await {
            Ok(metadata) => {
                if metadata.file_type == FileType::Directory {
                    return Err(Errno::from(libc::EISDIR));
                }
                
                // For now, we don't maintain file handles - just return success
                // In a more sophisticated implementation, we might cache file handles
                Ok((None, flags))
            }
            Err(RemoteFsError::NotFound(_)) => Err(Errno::from(libc::ENOENT)),
            Err(RemoteFsError::PermissionDenied(_)) => Err(Errno::from(libc::EACCES)),
            Err(e) => {
                error!("Failed to open {}: {}", remote_path.display(), e);
                Err(Errno::from(libc::EIO))
            }
        }
    }

    async fn read(
        &self,
        _req: Request,
        inode: u64,
        _fh: u64,
        offset: u64,
        size: u32,
    ) -> Result<Bytes> {
        debug!("read: inode={}, offset={}, size={}", inode, offset, size);
        
        let path = self.get_path_for_inode(inode).await
            .ok_or(Errno::from(libc::ENOENT))?;
        
        let remote_path = self.mount_to_remote_path(&path);
        
        match self.client.read_file_range(&remote_path, Some(offset), Some(size as u64)).await {
            Ok(data) => Ok(data),
            Err(RemoteFsError::NotFound(_)) => Err(Errno::from(libc::ENOENT)),
            Err(RemoteFsError::PermissionDenied(_)) => Err(Errno::from(libc::EACCES)),
            Err(e) => {
                error!("Failed to read {}: {}", remote_path.display(), e);
                Err(Errno::from(libc::EIO))
            }
        }
    }

    async fn write(
        &self,
        _req: Request,
        inode: u64,
        _fh: u64,
        offset: u64,
        data: &[u8],
        _flags: u32,
    ) -> Result<u32> {
        debug!("write: inode={}, offset={}, size={}", inode, offset, data.len());
        
        let path = self.get_path_for_inode(inode).await
            .ok_or(Errno::from(libc::ENOENT))?;
        
        let remote_path = self.mount_to_remote_path(&path);
        
        match self.client.write_file_at(&remote_path, Bytes::copy_from_slice(data), Some(offset), false).await {
            Ok(_) => {
                // Invalidate cached attributes since file has changed
                self.invalidate_attr_cache(inode).await;
                Ok(data.len() as u32)
            }
            Err(RemoteFsError::NotFound(_)) => Err(Errno::from(libc::ENOENT)),
            Err(RemoteFsError::PermissionDenied(_)) => Err(Errno::from(libc::EACCES)),
            Err(e) => {
                error!("Failed to write to {}: {}", remote_path.display(), e);
                Err(Errno::from(libc::EIO))
            }
        }
    }

    async fn create(
        &self,
        _req: Request,
        parent: u64,
        name: &OsStr,
        mode: u32,
        flags: u32,
    ) -> Result<(Duration, FileAttr, Option<u64>, u32)> {
        debug!("create: parent={}, name={:?}, mode={:o}, flags={:o}", parent, name, mode, flags);
        
        let parent_path = self.get_path_for_inode(parent).await
            .ok_or(Errno::from(libc::ENOENT))?;
        
        let child_path = parent_path.join(name);
        let remote_path = self.mount_to_remote_path(&child_path);
        
        match self.client.write_file(&remote_path, Bytes::new()).await {
            Ok(_) => {
                let inode = self.get_or_create_inode(&child_path).await;
                
                // Get attributes for the newly created file
                let attr = self.get_attr(inode).await?;
                
                Ok((ENTRY_TTL, attr, None, flags))
            }
            Err(RemoteFsError::PermissionDenied(_)) => Err(Errno::from(libc::EACCES)),
            Err(e) => {
                error!("Failed to create {}: {}", remote_path.display(), e);
                Err(Errno::from(libc::EIO))
            }
        }
    }

    async fn mkdir(
        &self,
        _req: Request,
        parent: u64,
        name: &OsStr,
        mode: u32,
    ) -> Result<(Duration, FileAttr)> {
        debug!("mkdir: parent={}, name={:?}, mode={:o}", parent, name, mode);
        
        let parent_path = self.get_path_for_inode(parent).await
            .ok_or(Errno::from(libc::ENOENT))?;
        
        let child_path = parent_path.join(name);
        let remote_path = self.mount_to_remote_path(&child_path);
        
        match self.client.create_directory(&remote_path).await {
            Ok(_) => {
                let inode = self.get_or_create_inode(&child_path).await;
                let attr = self.get_attr(inode).await?;
                Ok((ENTRY_TTL, attr))
            }
            Err(RemoteFsError::PermissionDenied(_)) => Err(Errno::from(libc::EACCES)),
            Err(RemoteFsError::AlreadyExists(_)) => Err(Errno::from(libc::EEXIST)),
            Err(e) => {
                error!("Failed to create directory {}: {}", remote_path.display(), e);
                Err(Errno::from(libc::EIO))
            }
        }
    }

    async fn unlink(&self, _req: Request, parent: u64, name: &OsStr) -> Result<()> {
        debug!("unlink: parent={}, name={:?}", parent, name);
        
        let parent_path = self.get_path_for_inode(parent).await
            .ok_or(Errno::from(libc::ENOENT))?;
        
        let child_path = parent_path.join(name);
        let remote_path = self.mount_to_remote_path(&child_path);
        
        match self.client.delete_file(&remote_path).await {
            Ok(_) => {
                // Remove from inode cache
                if let Some(inode) = {
                    let path_to_inode = self.path_to_inode.read().await;
                    path_to_inode.get(&child_path).copied()
                } {
                    let mut inode_to_path = self.inode_to_path.write().await;
                    let mut path_to_inode = self.path_to_inode.write().await;
                    inode_to_path.remove(&inode);
                    path_to_inode.remove(&child_path);
                    self.invalidate_attr_cache(inode).await;
                }
                Ok(())
            }
            Err(RemoteFsError::NotFound(_)) => Err(Errno::from(libc::ENOENT)),
            Err(RemoteFsError::PermissionDenied(_)) => Err(Errno::from(libc::EACCES)),
            Err(e) => {
                error!("Failed to unlink {}: {}", remote_path.display(), e);
                Err(Errno::from(libc::EIO))
            }
        }
    }

    async fn rmdir(&self, _req: Request, parent: u64, name: &OsStr) -> Result<()> {
        debug!("rmdir: parent={}, name={:?}", parent, name);
        
        let parent_path = self.get_path_for_inode(parent).await
            .ok_or(Errno::from(libc::ENOENT))?;
        
        let child_path = parent_path.join(name);
        let remote_path = self.mount_to_remote_path(&child_path);
        
        match self.client.delete_directory(&remote_path).await {
            Ok(_) => {
                // Remove from inode cache
                if let Some(inode) = {
                    let path_to_inode = self.path_to_inode.read().await;
                    path_to_inode.get(&child_path).copied()
                } {
                    let mut inode_to_path = self.inode_to_path.write().await;
                    let mut path_to_inode = self.path_to_inode.write().await;
                    inode_to_path.remove(&inode);
                    path_to_inode.remove(&child_path);
                    self.invalidate_attr_cache(inode).await;
                }
                Ok(())
            }
            Err(RemoteFsError::NotFound(_)) => Err(Errno::from(libc::ENOENT)),
            Err(RemoteFsError::PermissionDenied(_)) => Err(Errno::from(libc::EACCES)),
            Err(e) => {
                error!("Failed to remove directory {}: {}", remote_path.display(), e);
                Err(Errno::from(libc::EIO))
            }
        }
    }

    async fn rename(
        &self,
        _req: Request,
        parent: u64,
        name: &OsStr,
        new_parent: u64,
        new_name: &OsStr,
    ) -> Result<()> {
        debug!("rename: parent={}, name={:?}, new_parent={}, new_name={:?}", 
               parent, name, new_parent, new_name);
        
        let parent_path = self.get_path_for_inode(parent).await
            .ok_or(Errno::from(libc::ENOENT))?;
        
        let new_parent_path = self.get_path_for_inode(new_parent).await
            .ok_or(Errno::from(libc::ENOENT))?;
        
        let old_path = parent_path.join(name);
        let new_path = new_parent_path.join(new_name);
        
        let old_remote_path = self.mount_to_remote_path(&old_path);
        let new_remote_path = self.mount_to_remote_path(&new_path);
        
        match self.client.move_path(&old_remote_path, &new_remote_path).await {
            Ok(_) => {
                // Update inode cache
                if let Some(inode) = {
                    let path_to_inode = self.path_to_inode.read().await;
                    path_to_inode.get(&old_path).copied()
                } {
                    let mut inode_to_path = self.inode_to_path.write().await;
                    let mut path_to_inode = self.path_to_inode.write().await;
                    
                    path_to_inode.remove(&old_path);
                    inode_to_path.insert(inode, new_path.clone());
                    path_to_inode.insert(new_path, inode);
                    
                    self.invalidate_attr_cache(inode).await;
                }
                Ok(())
            }
            Err(RemoteFsError::NotFound(_)) => Err(Errno::from(libc::ENOENT)),
            Err(RemoteFsError::PermissionDenied(_)) => Err(Errno::from(libc::EACCES)),
            Err(e) => {
                error!("Failed to rename {} to {}: {}", old_remote_path.display(), new_remote_path.display(), e);
                Err(Errno::from(libc::EIO))
            }
        }
    }

    async fn flush(&self, _req: Request, inode: u64, _fh: u64) -> Result<()> {
        debug!("flush: inode={}", inode);
        // For now, we don't implement explicit flushing since writes go directly to remote
        // In a more sophisticated implementation, we might batch writes and flush them here
        Ok(())
    }

    async fn fsync(&self, _req: Request, inode: u64, _fh: u64, datasync: bool) -> Result<()> {
        debug!("fsync: inode={}, datasync={}", inode, datasync);
        // Similar to flush, we don't have local buffering to sync
        Ok(())
    }
}
