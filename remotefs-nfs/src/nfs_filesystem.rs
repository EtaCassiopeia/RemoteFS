use async_trait::async_trait;
use remotefs_client::{Client, ClientError};
use remotefs_common::{
    protocol::{FileMetadata, Message},
    error::RemoteFsError,
};
use std::collections::HashMap;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use tokio::sync::RwLock;
use tracing::{debug, warn};
use zerofs_nfsserve::{
    nfs::{fattr3, fileid3, filename3, ftype3, nfsstat3, nfspath3, sattr3, set_atime, set_gid3, set_mode3, set_mtime, set_size3, set_uid3, nfstime3, specdata3},
    vfs::{VFSCapabilities, NFSFileSystem, AuthContext, ReadDirResult, DirEntry as NfsDirEntry},
};

/// NFS filesystem adapter that proxies requests to RemoteFS agents
pub struct RemoteNfsFilesystem {
    pub client: Arc<Client>,
    pub next_file_id: AtomicU64,
    pub path_to_id_map: Arc<RwLock<HashMap<String, u64>>>,
    pub id_to_path_map: Arc<RwLock<HashMap<u64, String>>>,
    pub root_id: u64,
}

impl RemoteNfsFilesystem {
    pub async fn new(client: Client) -> crate::Result<Self> {
        let root_id = 1u64; // NFS root directory has ID 1
        let mut path_to_id_map = HashMap::new();
        let mut id_to_path_map = HashMap::new();
        
        // Map root directory
        path_to_id_map.insert("/".to_string(), root_id);
        id_to_path_map.insert(root_id, "/".to_string());
        
        Ok(Self {
            client: Arc::new(client),
            next_file_id: AtomicU64::new(root_id + 1),
            path_to_id_map: Arc::new(RwLock::new(path_to_id_map)),
            id_to_path_map: Arc::new(RwLock::new(id_to_path_map)),
            root_id,
        })
    }
    
    /// Get or create a file ID for the given path
    async fn get_or_create_file_id(&self, path: &str) -> u64 {
        let normalized_path = self.normalize_path(path);
        
        // First try to get existing ID
        {
            let path_map = self.path_to_id_map.read().await;
            if let Some(&id) = path_map.get(&normalized_path) {
                return id;
            }
        }
        
        // Create new ID
        let new_id = self.next_file_id.fetch_add(1, Ordering::SeqCst);
        
        // Update both maps
        {
            let mut path_map = self.path_to_id_map.write().await;
            let mut id_map = self.id_to_path_map.write().await;
            
            path_map.insert(normalized_path.clone(), new_id);
            id_map.insert(new_id, normalized_path);
        }
        
        new_id
    }
    
    /// Get the path for a file ID
    async fn get_path_for_id(&self, id: u64) -> Option<String> {
        let id_map = self.id_to_path_map.read().await;
        id_map.get(&id).cloned()
    }
    
    /// Normalize a path for consistent handling
    fn normalize_path(&self, path: &str) -> String {
        if path.is_empty() || path == "/" {
            "/".to_string()
        } else {
            // Remove trailing slash unless it's the root
            let mut normalized = path.trim_end_matches('/').to_string();
            if !normalized.starts_with('/') {
                normalized = format!("/{}", normalized);
            }
            normalized
        }
    }
    
    /// Join directory and filename to create full path
    fn join_path(&self, dir_path: &str, filename: &str) -> String {
        if dir_path == "/" {
            format!("/{}", filename)
        } else {
            format!("{}/{}", dir_path, filename)
        }
    }
    
    /// Convert FileMetadata to NFS file attributes
    fn file_metadata_to_fattr(&self, metadata: &FileMetadata, file_id: u64) -> fattr3 {
        let file_type = if metadata.is_dir {
            ftype3::NF3DIR
        } else {
            ftype3::NF3REG
        };
        
        fattr3 {
            ftype: file_type,
            mode: metadata.permissions,
            nlink: 1,
            uid: 1000, // Default UID
            gid: 1000, // Default GID
            size: metadata.size,
            used: metadata.size,
            rdev: specdata3 { specdata1: 0, specdata2: 0 },
            fsid: 1,
            fileid: file_id,
            atime: nfstime3 {
                seconds: metadata.accessed.timestamp() as u32,
                nseconds: metadata.accessed.timestamp_subsec_nanos(),
            },
            mtime: nfstime3 {
                seconds: metadata.modified.timestamp() as u32,
                nseconds: metadata.modified.timestamp_subsec_nanos(),
            },
            ctime: nfstime3 {
                seconds: metadata.created.timestamp() as u32,
                nseconds: metadata.created.timestamp_subsec_nanos(),
            },
        }
    }
}

#[async_trait]
impl NFSFileSystem for RemoteNfsFilesystem {
    fn root_dir(&self) -> fileid3 {
        self.root_id
    }

    fn capabilities(&self) -> VFSCapabilities {
        VFSCapabilities::ReadWrite
    }

    async fn lookup(
        &self,
        _auth: &AuthContext,
        dirid: fileid3,
        filename: &filename3,
    ) -> Result<fileid3, nfsstat3> {
        debug!("NFS lookup: dirid={}, filename={:?}", dirid, String::from_utf8_lossy(filename));
        
        // Get directory path
        let dir_path = match self.get_path_for_id(dirid).await {
            Some(path) => path,
            None => {
                debug!("Directory ID {} not found", dirid);
                return Err(nfsstat3::NFS3ERR_NOENT);
            }
        };
        
        let filename_str = String::from_utf8_lossy(filename);
        let full_path = self.join_path(&dir_path, &filename_str);
        
        debug!("Looking up full path: {}", full_path);
        
        // Try to get metadata to verify file exists
        match self.client.get_metadata_with_options(&full_path, false).await {
            Ok(_) => {
                let file_id = self.get_or_create_file_id(&full_path).await;
                debug!("Lookup successful: {} -> {}", full_path, file_id);
                Ok(file_id)
            }
            Err(ClientError::RemoteFs(RemoteFsError::NotFound(_))) => {
                debug!("File not found: {}", full_path);
                Err(nfsstat3::NFS3ERR_NOENT)
            }
            Err(e) => {
                warn!("Lookup error for {}: {:?}", full_path, e);
                Err(nfsstat3::NFS3ERR_IO)
            }
        }
    }

    async fn getattr(&self, _auth: &AuthContext, id: fileid3) -> Result<fattr3, nfsstat3> {
        debug!("NFS getattr: id={}", id);
        
        let path = match self.get_path_for_id(id).await {
            Some(path) => path,
            None => {
                debug!("File ID {} not found in getattr", id);
                return Err(nfsstat3::NFS3ERR_NOENT);
            }
        };
        
        match self.client.get_metadata_with_options(&path, false).await {
            Ok(metadata) => {
                let fattr = self.file_metadata_to_fattr(&metadata, id);
                debug!("getattr successful for {}: {:?}", path, fattr);
                Ok(fattr)
            }
            Err(ClientError::RemoteFs(RemoteFsError::NotFound(_))) => Err(nfsstat3::NFS3ERR_NOENT),
            Err(e) => {
                warn!("getattr error for {}: {:?}", path, e);
                Err(nfsstat3::NFS3ERR_IO)
            }
        }
    }

    async fn read(
        &self,
        _auth: &AuthContext,
        id: fileid3,
        offset: u64,
        count: u32,
    ) -> Result<(Vec<u8>, bool), nfsstat3> {
        debug!("NFS read: id={}, offset={}, count={}", id, offset, count);
        
        let path = match self.get_path_for_id(id).await {
            Some(path) => path,
            None => return Err(nfsstat3::NFS3ERR_NOENT),
        };
        
        match self.client.read_file_range(&path, Some(offset), Some(count as u64)).await {
            Ok(data) => {
                let eof = (data.len() as u32) < count;
                debug!("Read {} bytes from {}, eof={}", data.len(), path, eof);
                Ok((data.to_vec(), eof))
            }
            Err(ClientError::RemoteFs(RemoteFsError::NotFound(_))) => Err(nfsstat3::NFS3ERR_NOENT),
            Err(ClientError::RemoteFs(RemoteFsError::PermissionDenied(_))) => Err(nfsstat3::NFS3ERR_ACCES),
            Err(e) => {
                warn!("Read error for {}: {:?}", path, e);
                Err(nfsstat3::NFS3ERR_IO)
            }
        }
    }

    async fn write(
        &self,
        _auth: &AuthContext,
        id: fileid3,
        offset: u64,
        data: &[u8],
    ) -> Result<fattr3, nfsstat3> {
        debug!("NFS write: id={}, offset={}, len={}", id, offset, data.len());
        
        let path = match self.get_path_for_id(id).await {
            Some(path) => path,
            None => return Err(nfsstat3::NFS3ERR_NOENT),
        };
        
        match self.client.write_file_at(&path, bytes::Bytes::from(data.to_vec()), Some(offset), false).await {
            Ok(_) => {
                // Get updated metadata
                match self.client.get_metadata_with_options(&path, false).await {
                    Ok(metadata) => {
                        let fattr = self.file_metadata_to_fattr(&metadata, id);
                        debug!("Write successful for {}", path);
                        Ok(fattr)
                    }
                    Err(_) => Err(nfsstat3::NFS3ERR_IO),
                }
            }
            Err(ClientError::RemoteFs(RemoteFsError::NotFound(_))) => Err(nfsstat3::NFS3ERR_NOENT),
            Err(ClientError::RemoteFs(RemoteFsError::PermissionDenied(_))) => Err(nfsstat3::NFS3ERR_ACCES),
            Err(e) => {
                warn!("Write error for {}: {:?}", path, e);
                Err(nfsstat3::NFS3ERR_IO)
            }
        }
    }

    async fn create(
        &self,
        _auth: &AuthContext,
        dirid: fileid3,
        filename: &filename3,
        _attr: sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        debug!("NFS create: dirid={}, filename={:?}", dirid, String::from_utf8_lossy(filename));
        
        let dir_path = match self.get_path_for_id(dirid).await {
            Some(path) => path,
            None => return Err(nfsstat3::NFS3ERR_NOENT),
        };
        
        let filename_str = String::from_utf8_lossy(filename);
        let full_path = self.join_path(&dir_path, &filename_str);
        
        match self.client.write_file(&full_path, bytes::Bytes::new()).await {
            Ok(_) => {
                let file_id = self.get_or_create_file_id(&full_path).await;
                
                // Get file metadata
                match self.client.get_metadata_with_options(&full_path, false).await {
                    Ok(metadata) => {
                        let fattr = self.file_metadata_to_fattr(&metadata, file_id);
                        debug!("Create successful: {} -> {}", full_path, file_id);
                        Ok((file_id, fattr))
                    }
                    Err(_) => Err(nfsstat3::NFS3ERR_IO),
                }
            }
            Err(ClientError::RemoteFs(RemoteFsError::AlreadyExists(_))) => Err(nfsstat3::NFS3ERR_EXIST),
            Err(ClientError::RemoteFs(RemoteFsError::PermissionDenied(_))) => Err(nfsstat3::NFS3ERR_ACCES),
            Err(e) => {
                warn!("Create error for {}: {:?}", full_path, e);
                Err(nfsstat3::NFS3ERR_IO)
            }
        }
    }

    async fn mkdir(
        &self,
        _auth: &AuthContext,
        dirid: fileid3,
        dirname: &filename3,
        _attr: &sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        debug!("NFS mkdir: dirid={}, dirname={:?}", dirid, String::from_utf8_lossy(dirname));
        
        let dir_path = match self.get_path_for_id(dirid).await {
            Some(path) => path,
            None => return Err(nfsstat3::NFS3ERR_NOENT),
        };
        
        let dirname_str = String::from_utf8_lossy(dirname);
        let full_path = self.join_path(&dir_path, &dirname_str);
        
        match self.client.create_directory(&full_path).await {
            Ok(_) => {
                let dir_id = self.get_or_create_file_id(&full_path).await;
                
                // Get directory metadata
                match self.client.get_metadata_with_options(&full_path, false).await {
                    Ok(metadata) => {
                        let fattr = self.file_metadata_to_fattr(&metadata, dir_id);
                        debug!("Mkdir successful: {} -> {}", full_path, dir_id);
                        Ok((dir_id, fattr))
                    }
                    Err(_) => Err(nfsstat3::NFS3ERR_IO),
                }
            }
            Err(ClientError::RemoteFs(RemoteFsError::AlreadyExists(_))) => Err(nfsstat3::NFS3ERR_EXIST),
            Err(ClientError::RemoteFs(RemoteFsError::PermissionDenied(_))) => Err(nfsstat3::NFS3ERR_ACCES),
            Err(e) => {
                warn!("Mkdir error for {}: {:?}", full_path, e);
                Err(nfsstat3::NFS3ERR_IO)
            }
        }
    }

    async fn remove(
        &self,
        _auth: &AuthContext,
        dirid: fileid3,
        filename: &filename3,
    ) -> Result<(), nfsstat3> {
        debug!("NFS remove: dirid={}, filename={:?}", dirid, String::from_utf8_lossy(filename));
        
        let dir_path = match self.get_path_for_id(dirid).await {
            Some(path) => path,
            None => return Err(nfsstat3::NFS3ERR_NOENT),
        };
        
        let filename_str = String::from_utf8_lossy(filename);
        let full_path = self.join_path(&dir_path, &filename_str);
        
        match self.client.delete_file(&full_path).await {
            Ok(_) => {
                // Remove from our mappings
                {
                    let mut path_map = self.path_to_id_map.write().await;
                    let mut id_map = self.id_to_path_map.write().await;
                    
                    if let Some(id) = path_map.remove(&full_path) {
                        id_map.remove(&id);
                    }
                }
                debug!("Remove successful: {}", full_path);
                Ok(())
            }
            Err(ClientError::RemoteFs(RemoteFsError::NotFound(_))) => Err(nfsstat3::NFS3ERR_NOENT),
            Err(ClientError::RemoteFs(RemoteFsError::PermissionDenied(_))) => Err(nfsstat3::NFS3ERR_ACCES),
            Err(e) => {
                warn!("Remove error for {}: {:?}", full_path, e);
                Err(nfsstat3::NFS3ERR_IO)
            }
        }
    }

    async fn readdir(
        &self,
        _auth: &AuthContext,
        dirid: fileid3,
        start_after: fileid3,
        max_entries: usize,
    ) -> Result<ReadDirResult, nfsstat3> {
        debug!("NFS readdir: dirid={}, start_after={}, max_entries={}", dirid, start_after, max_entries);
        
        let dir_path = match self.get_path_for_id(dirid).await {
            Some(path) => path,
            None => return Err(nfsstat3::NFS3ERR_NOENT),
        };
        
        match self.client.list_directory(&dir_path).await {
            Ok(entries) => {
                let mut nfs_entries = Vec::new();
                let mut count = 0;
                
                // Add . and .. entries for NFS compatibility
                if start_after == 0 {
                    // Add . entry
                    if count < max_entries {
                        if let Ok(metadata) = self.client.get_metadata(&dir_path).await {
                            let fattr = self.file_metadata_to_fattr(&metadata, dirid);
                        nfs_entries.push(NfsDirEntry {
                            fileid: dirid,
                            name: zerofs_nfsserve::nfs::nfsstring(b".".to_vec()),
                            attr: fattr,
                        });
                            count += 1;
                        }
                    }
                    
                    // Add .. entry (parent directory)
                    if count < max_entries {
                        let parent_path = if dir_path == "/" {
                            "/".to_string()
                        } else {
                            let parent = std::path::Path::new(&dir_path).parent()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|| "/".to_string());
                            if parent.is_empty() { "/".to_string() } else { parent }
                        };
                        
                        let parent_id = self.get_or_create_file_id(&parent_path).await;
                        if let Ok(metadata) = self.client.get_metadata(&parent_path).await {
                            let fattr = self.file_metadata_to_fattr(&metadata, parent_id);
                            nfs_entries.push(NfsDirEntry {
                                fileid: parent_id,
                                name: zerofs_nfsserve::nfs::nfsstring(b"..".to_vec()),
                                attr: fattr,
                            });
                            count += 1;
                        }
                    }
                }
                
                // Add directory entries
                for entry in entries.into_iter().skip(if start_after == 0 { 0 } else { start_after as usize }) {
                    if count >= max_entries {
                        break;
                    }
                    
                    let entry_path = self.join_path(&dir_path, &entry.name);
                    let entry_id = self.get_or_create_file_id(&entry_path).await;
                    
                    let fattr = self.file_metadata_to_fattr(&entry.metadata, entry_id);
                    nfs_entries.push(NfsDirEntry {
                        fileid: entry_id,
                        name: zerofs_nfsserve::nfs::nfsstring(entry.name.into_bytes()),
                        attr: fattr,
                    });
                    count += 1;
                }
                
                debug!("Readdir successful: {} entries returned", nfs_entries.len());
                Ok(ReadDirResult {
                    entries: nfs_entries,
                    end: count < max_entries,
                })
            }
            Err(ClientError::RemoteFs(RemoteFsError::NotFound(_))) => Err(nfsstat3::NFS3ERR_NOENT),
            Err(ClientError::RemoteFs(RemoteFsError::PermissionDenied(_))) => Err(nfsstat3::NFS3ERR_ACCES),
            Err(e) => {
                warn!("Readdir error for {}: {:?}", dir_path, e);
                Err(nfsstat3::NFS3ERR_IO)
            }
        }
    }

    // Implement additional NFS operations as needed
    async fn rename(
        &self,
        _auth: &AuthContext,
        from_dirid: fileid3,
        from_filename: &filename3,
        to_dirid: fileid3,
        to_filename: &filename3,
    ) -> Result<(), nfsstat3> {
        debug!("NFS rename: from_dirid={}, to_dirid={}", from_dirid, to_dirid);
        
        let from_dir_path = match self.get_path_for_id(from_dirid).await {
            Some(path) => path,
            None => return Err(nfsstat3::NFS3ERR_NOENT),
        };
        
        let to_dir_path = match self.get_path_for_id(to_dirid).await {
            Some(path) => path,
            None => return Err(nfsstat3::NFS3ERR_NOENT),
        };
        
        let from_filename_str = String::from_utf8_lossy(from_filename);
        let to_filename_str = String::from_utf8_lossy(to_filename);
        let from_path = self.join_path(&from_dir_path, &from_filename_str);
        let to_path = self.join_path(&to_dir_path, &to_filename_str);
        
        match self.client.move_path(&from_path, &to_path).await {
            Ok(_) => {
                // Update our path mappings
                {
                    let mut path_map = self.path_to_id_map.write().await;
                    let mut id_map = self.id_to_path_map.write().await;
                    
                    if let Some(id) = path_map.remove(&from_path) {
                        path_map.insert(to_path.clone(), id);
                        id_map.insert(id, to_path.clone());
                    }
                }
                debug!("Rename successful: {} -> {}", from_path, to_path);
                Ok(())
            }
            Err(ClientError::RemoteFs(RemoteFsError::NotFound(_))) => Err(nfsstat3::NFS3ERR_NOENT),
            Err(ClientError::RemoteFs(RemoteFsError::PermissionDenied(_))) => Err(nfsstat3::NFS3ERR_ACCES),
            Err(e) => {
                warn!("Rename error {} -> {}: {:?}", from_path, to_path, e);
                Err(nfsstat3::NFS3ERR_IO)
            }
        }
    }

    async fn setattr(
        &self,
        _auth: &AuthContext,
        _id: fileid3,
        _setattr: sattr3,
    ) -> Result<fattr3, nfsstat3> {
        // For now, return the current attributes without making changes
        // This could be extended to support permission changes, etc.
        self.getattr(_auth, _id).await
    }

    // Stub implementations for less common operations
    async fn create_exclusive(
        &self,
        auth: &AuthContext,
        dirid: fileid3,
        filename: &filename3,
    ) -> Result<fileid3, nfsstat3> {
        // For exclusive create, just use regular create for now
        match self.create(auth, dirid, filename, sattr3::default()).await {
            Ok((fileid, _)) => Ok(fileid),
            Err(e) => Err(e),
        }
    }

    async fn symlink(
        &self,
        _auth: &AuthContext,
        _dirid: fileid3,
        _linkname: &filename3,
        _symlink: &nfspath3,
        _attr: &sattr3,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        // Symlinks not supported for now
        Err(nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn readlink(&self, _auth: &AuthContext, _id: fileid3) -> Result<nfspath3, nfsstat3> {
        // Symlinks not supported for now
        Err(nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn mknod(
        &self,
        _auth: &AuthContext,
        _dirid: fileid3,
        _filename: &filename3,
        _ftype: ftype3,
        _attr: &sattr3,
        _spec: Option<&specdata3>,
    ) -> Result<(fileid3, fattr3), nfsstat3> {
        // Special files not supported
        Err(nfsstat3::NFS3ERR_NOTSUPP)
    }

    async fn link(
        &self,
        _auth: &AuthContext,
        _id: fileid3,
        _dirid: fileid3,
        _filename: &filename3,
    ) -> Result<(), nfsstat3> {
        // Hard links not supported for now
        Err(nfsstat3::NFS3ERR_NOTSUPP)
    }
}
