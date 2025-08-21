use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::error::{RemoteFsError, Result};

/// Path utilities
pub mod path {
    use super::*;
    
    /// Normalize a path to prevent directory traversal attacks
    pub fn normalize_path(path: &str) -> Result<PathBuf> {
        let path = Path::new(path);
        let mut normalized = PathBuf::new();
        
        for component in path.components() {
            match component {
                std::path::Component::Prefix(_) => {
                    return Err(RemoteFsError::InvalidPath(
                        "Absolute paths with prefixes not allowed".to_string()
                    ));
                }
                std::path::Component::RootDir => {
                    normalized.push("/");
                }
                std::path::Component::CurDir => {
                    // Skip current directory references
                }
                std::path::Component::ParentDir => {
                    if !normalized.pop() {
                        return Err(RemoteFsError::InvalidPath(
                            "Path traversal above root not allowed".to_string()
                        ));
                    }
                }
                std::path::Component::Normal(name) => {
                    normalized.push(name);
                }
            }
        }
        
        Ok(normalized)
    }
    
    /// Check if a path is within allowed bounds
    pub fn is_path_allowed(path: &Path, allowed_paths: &[String]) -> bool {
        if allowed_paths.is_empty() {
            return true; // No restrictions
        }
        
        for allowed in allowed_paths {
            let allowed_path = Path::new(allowed);
            if path.starts_with(allowed_path) {
                return true;
            }
        }
        
        false
    }
    
    /// Check if a path is denied
    pub fn is_path_denied(path: &Path, denied_paths: &[String]) -> bool {
        for denied in denied_paths {
            let denied_path = Path::new(denied);
            if path.starts_with(denied_path) {
                return true;
            }
        }
        
        false
    }
    
    /// Check if a path has an allowed extension
    pub fn has_allowed_extension(path: &Path, allowed_extensions: &[String]) -> bool {
        if allowed_extensions.is_empty() {
            return true; // No restrictions
        }
        
        if let Some(extension) = path.extension() {
            if let Some(ext_str) = extension.to_str() {
                return allowed_extensions.iter().any(|allowed| allowed == ext_str);
            }
        }
        
        false
    }
    
    /// Check if a path has a denied extension
    pub fn has_denied_extension(path: &Path, denied_extensions: &[String]) -> bool {
        if let Some(extension) = path.extension() {
            if let Some(ext_str) = extension.to_str() {
                return denied_extensions.iter().any(|denied| denied == ext_str);
            }
        }
        
        false
    }
    
    /// Join paths safely
    pub fn safe_join(base: &Path, path: &str) -> Result<PathBuf> {
        let normalized = normalize_path(path)?;
        
        // Remove leading slash if present to make it relative
        let relative_path = normalized.strip_prefix("/").unwrap_or(&normalized);
        
        Ok(base.join(relative_path))
    }
}

/// Time utilities
pub mod time {
    use super::*;
    use chrono::{DateTime, Utc};
    
    /// Get current timestamp as seconds since Unix epoch
    pub fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
    
    /// Get current timestamp as milliseconds since Unix epoch
    pub fn current_timestamp_millis() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
    
    /// Convert system time to chrono DateTime
    pub fn system_time_to_datetime(time: SystemTime) -> DateTime<Utc> {
        DateTime::from(time)
    }
    
    /// Convert chrono DateTime to system time
    pub fn datetime_to_system_time(datetime: DateTime<Utc>) -> SystemTime {
        datetime.into()
    }
    
    /// Check if a timestamp is expired given a TTL
    pub fn is_expired(timestamp: u64, ttl_seconds: u64) -> bool {
        let current = current_timestamp();
        current > timestamp + ttl_seconds
    }
}

/// Bytes utilities
pub mod bytes {
    use super::*;
    
    /// Format bytes as human readable string
    pub fn format_bytes(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        
        if bytes == 0 {
            return "0 B".to_string();
        }
        
        let mut size = bytes as f64;
        let mut unit_index = 0;
        
        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }
        
        format!("{:.1} {}", size, UNITS[unit_index])
    }
    
    /// Parse human readable byte string to u64
    pub fn parse_bytes(input: &str) -> Result<u64> {
        let input = input.trim().to_lowercase();
        
        let (number_str, unit) = if let Some(pos) = input.find(|c: char| c.is_alphabetic()) {
            input.split_at(pos)
        } else {
            (input.as_str(), "")
        };
        
        let number: f64 = number_str.trim().parse()
            .map_err(|_| RemoteFsError::InvalidPath(format!("Invalid byte value: {}", input)))?;
            
        let multiplier = match unit.trim() {
            "" | "b" => 1,
            "kb" | "k" => 1024,
            "mb" | "m" => 1024 * 1024,
            "gb" | "g" => 1024 * 1024 * 1024,
            "tb" | "t" => 1024_u64.pow(4),
            _ => return Err(RemoteFsError::InvalidPath(format!("Unknown byte unit: {}", unit))),
        };
        
        Ok((number * multiplier as f64) as u64)
    }
    
    /// Calculate hash of byte slice
    pub fn hash_bytes(data: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish()
    }
}

/// Network utilities
pub mod network {
    use super::*;
    use std::net::{IpAddr, SocketAddr};
    
    /// Parse a URL-like string into components
    pub fn parse_url(url: &str) -> Result<(String, String, u16, String)> {
        let url = url::Url::parse(url)
            .map_err(|e| RemoteFsError::InvalidPath(format!("Invalid URL: {}", e)))?;
            
        let scheme = url.scheme().to_string();
        let host = url.host_str()
            .ok_or_else(|| RemoteFsError::InvalidPath("No host in URL".to_string()))?
            .to_string();
        let port = url.port().unwrap_or(match scheme.as_str() {
            "ws" | "http" => 80,
            "wss" | "https" => 443,
            _ => 8080,
        });
        let path = url.path().to_string();
        
        Ok((scheme, host, port, path))
    }
    
    /// Check if an IP address is loopback
    pub fn is_loopback_addr(addr: &SocketAddr) -> bool {
        match addr.ip() {
            IpAddr::V4(ipv4) => ipv4.is_loopback(),
            IpAddr::V6(ipv6) => ipv6.is_loopback(),
        }
    }
    
    /// Check if an IP address is private
    pub fn is_private_addr(addr: &SocketAddr) -> bool {
        match addr.ip() {
            IpAddr::V4(ipv4) => ipv4.is_private(),
            IpAddr::V6(ipv6) => {
                // IPv6 private address ranges
                let octets = ipv6.octets();
                // fc00::/7 - unique local addresses
                (octets[0] & 0xfe) == 0xfc
            }
        }
    }
}

/// Retry utilities
pub mod retry {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;
    
    /// Exponential backoff configuration
    #[derive(Debug, Clone)]
    pub struct BackoffConfig {
        pub initial_delay: Duration,
        pub max_delay: Duration,
        pub multiplier: f64,
        pub max_attempts: u32,
    }
    
    impl Default for BackoffConfig {
        fn default() -> Self {
            Self {
                initial_delay: Duration::from_millis(100),
                max_delay: Duration::from_secs(60),
                multiplier: 2.0,
                max_attempts: 5,
            }
        }
    }
    
    /// Retry a future with exponential backoff
    pub async fn retry_with_backoff<F, Fut, T, E>(
        mut operation: F,
        config: BackoffConfig,
    ) -> std::result::Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = std::result::Result<T, E>>,
        E: std::fmt::Debug,
    {
        let mut delay = config.initial_delay;
        let mut attempts = 0;
        
        loop {
            attempts += 1;
            
            match operation().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    if attempts >= config.max_attempts {
                        return Err(error);
                    }
                    
                    tracing::warn!(
                        "Operation failed (attempt {}/{}), retrying after {:?}: {:?}",
                        attempts, config.max_attempts, delay, error
                    );
                    
                    sleep(delay).await;
                    
                    delay = Duration::from_millis(
                        ((delay.as_millis() as f64) * config.multiplier) as u64
                    ).min(config.max_delay);
                }
            }
        }
    }
}

/// Validation utilities
pub mod validation {
    use super::*;
    
    /// Validate a node ID
    pub fn validate_node_id(id: &str) -> Result<()> {
        if id.is_empty() {
            return Err(RemoteFsError::InvalidPath("Node ID cannot be empty".to_string()));
        }
        
        if id.len() > 64 {
            return Err(RemoteFsError::InvalidPath("Node ID too long (max 64 characters)".to_string()));
        }
        
        if !id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(RemoteFsError::InvalidPath(
                "Node ID can only contain alphanumeric characters, hyphens, and underscores".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Validate a file path
    pub fn validate_file_path(path: &str) -> Result<()> {
        if path.is_empty() {
            return Err(RemoteFsError::InvalidPath("Path cannot be empty".to_string()));
        }
        
        if path.len() > 4096 {
            return Err(RemoteFsError::InvalidPath("Path too long (max 4096 characters)".to_string()));
        }
        
        // Check for null bytes
        if path.contains('\0') {
            return Err(RemoteFsError::InvalidPath("Path cannot contain null bytes".to_string()));
        }
        
        Ok(())
    }
    
    /// Validate message size
    pub fn validate_message_size(size: usize, max_size: usize) -> Result<()> {
        if size > max_size {
            return Err(RemoteFsError::Protocol(
                format!("Message size {} exceeds maximum {}", size, max_size)
            ));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_path_normalization() {
        assert_eq!(
            path::normalize_path("/test/../safe/path").unwrap(),
            PathBuf::from("/safe/path")
        );
        
        assert_eq!(
            path::normalize_path("./relative/path").unwrap(),
            PathBuf::from("relative/path")
        );
        
        assert!(path::normalize_path("../../../etc/passwd").is_err());
    }
    
    #[test]
    fn test_bytes_formatting() {
        assert_eq!(bytes::format_bytes(0), "0 B");
        assert_eq!(bytes::format_bytes(1024), "1.0 KB");
        assert_eq!(bytes::format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(bytes::format_bytes(1536 * 1024 * 1024), "1.5 GB");
    }
    
    #[test]
    fn test_bytes_parsing() {
        assert_eq!(bytes::parse_bytes("1024").unwrap(), 1024);
        assert_eq!(bytes::parse_bytes("1 KB").unwrap(), 1024);
        assert_eq!(bytes::parse_bytes("1.5 MB").unwrap(), (1.5 * 1024.0 * 1024.0) as u64);
        assert_eq!(bytes::parse_bytes("2 GB").unwrap(), 2 * 1024 * 1024 * 1024);
    }
    
    #[test]
    fn test_validation() {
        assert!(validation::validate_node_id("client-123").is_ok());
        assert!(validation::validate_node_id("agent_server").is_ok());
        assert!(validation::validate_node_id("").is_err());
        assert!(validation::validate_node_id("invalid@id").is_err());
        
        assert!(validation::validate_file_path("/valid/path").is_ok());
        assert!(validation::validate_file_path("").is_err());
        assert!(validation::validate_file_path("path\0with\0null").is_err());
    }
    
    #[test]
    fn test_url_parsing() {
        let (scheme, host, port, path) = network::parse_url("wss://example.com:8080/ws").unwrap();
        assert_eq!(scheme, "wss");
        assert_eq!(host, "example.com");
        assert_eq!(port, 8080);
        assert_eq!(path, "/ws");
    }
}
