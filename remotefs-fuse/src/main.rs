use anyhow::{Context, Result};
use clap::{Arg, Command};
use remotefs_fuse::{
    config::{load_fuse_config, FuseConfig},
    mount::mount_filesystem,
};
use std::path::PathBuf;
use tracing::{info, warn};
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("remotefs-mount")
        .version(env!("CARGO_PKG_VERSION"))
        .about("RemoteFS FUSE mount utility")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .required(true),
        )
        .arg(
            Arg::new("log-level")
                .short('l')
                .long("log-level")
                .value_name("LEVEL")
                .help("Log level (trace, debug, info, warn, error)")
                .default_value("info"),
        )
        .arg(
            Arg::new("daemonize")
                .short('d')
                .long("daemon")
                .help("Run as daemon in background")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    // Initialize logging
    let log_level = matches.get_one::<String>("log-level").unwrap();
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("remotefs_fuse={}", log_level)));

    fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .init();

    info!("RemoteFS FUSE Mount starting up");

    // Load configuration
    let config_path = matches.get_one::<String>("config").unwrap();
    let config_path = PathBuf::from(config_path);

    info!("Loading configuration from: {}", config_path.display());
    let config = load_fuse_config(&config_path)
        .with_context(|| format!("Failed to load configuration from {}", config_path.display()))?;

    info!("Configuration loaded successfully");
    info!("Client ID: {}", config.client.client_id);
    info!("Mount points: {}", config.mounts.len());

    // Validate mount points exist
    for (i, mount) in config.mounts.iter().enumerate() {
        if !mount.mount_path.exists() {
            warn!(
                "Mount point {} does not exist: {}",
                i,
                mount.mount_path.display()
            );
            info!("Creating mount point directory: {}", mount.mount_path.display());
            std::fs::create_dir_all(&mount.mount_path).with_context(|| {
                format!("Failed to create mount directory: {}", mount.mount_path.display())
            })?;
        }
    }

    // Check if should daemonize
    if matches.get_flag("daemonize") {
        info!("Daemonizing process");
        daemonize()?;
    }

    // Mount filesystems
    info!("Starting FUSE mounts...");
    let mount_handles = mount_filesystem(config).await.with_context(|| "Failed to mount filesystems")?;

    info!("All mount points initialized successfully");
    info!("Press Ctrl+C to unmount and exit");

    // Wait for shutdown signal
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            info!("Received Ctrl+C, shutting down...");
        }
        Err(err) => {
            warn!("Failed to listen for shutdown signal: {}", err);
        }
    }

    // Cleanup - mount handles should auto-unmount
    info!("Cleaning up mount points...");
    drop(mount_handles);

    info!("RemoteFS FUSE Mount shutdown complete");
    Ok(())
}

#[cfg(unix)]
fn daemonize() -> Result<()> {
    use std::process;

    match unsafe { libc::fork() } {
        -1 => anyhow::bail!("Failed to fork process"),
        0 => {
            // Child process - continue execution
        }
        _ => {
            // Parent process - exit
            process::exit(0);
        }
    }

    // Create new session
    if unsafe { libc::setsid() } == -1 {
        anyhow::bail!("Failed to create new session");
    }

    // Fork again to ensure we can't regain controlling terminal
    match unsafe { libc::fork() } {
        -1 => anyhow::bail!("Failed to fork process second time"),
        0 => {
            // Grandchild process - continue execution
        }
        _ => {
            // Child process - exit
            process::exit(0);
        }
    }

    // Change working directory to root
    std::env::set_current_dir("/").context("Failed to change directory to root")?;

    // Close standard file descriptors
    unsafe {
        libc::close(0);
        libc::close(1);
        libc::close(2);
    }

    // Redirect to /dev/null
    let dev_null = std::ffi::CString::new("/dev/null")?;
    unsafe {
        libc::open(dev_null.as_ptr(), libc::O_RDWR);
        libc::dup(0);
        libc::dup(0);
    }

    Ok(())
}

#[cfg(not(unix))]
fn daemonize() -> Result<()> {
    anyhow::bail!("Daemonization is only supported on Unix systems");
}
