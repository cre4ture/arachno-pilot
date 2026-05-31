use std::{
    fmt,
    fs::{self, OpenOptions},
    io,
    os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

use anyhow::Context;
use arachno_camera::RobotCamera;
use arachno_core::{CameraBackend, RobotConfig};
use arachno_hal::CameraSource;
use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "config/robot/default.toml")]
    config: PathBuf,
}

#[derive(Debug)]
struct DeviceProbe {
    label: &'static str,
    configured_path: PathBuf,
    resolved_path: Option<PathBuf>,
    kind: String,
    mode: u32,
    owner_uid: u32,
    owner_gid: u32,
    open_result: Result<(), io::Error>,
    hint: Option<&'static str>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config_text = fs::read_to_string(&args.config)
        .with_context(|| format!("failed to read {}", args.config.display()))?;
    let config: RobotConfig = toml::from_str(&config_text)
        .with_context(|| format!("failed to parse {}", args.config.display()))?;

    println!("robot: {}", config.robot.name);
    println!("deployment_profile: {}", config.deployment.profile);
    println!("compute_target: {}", config.deployment.compute);
    println!("servo_port: {}", config.bus.feetech.port);

    let camera = RobotCamera::new(config.camera.clone());
    println!("camera_backend: {:?}", config.camera.backend);
    println!("camera_pipeline: {}", camera.pipeline_description());
    println!();

    let serial_probe = probe_serial(Path::new(&config.bus.feetech.port));
    print_probe(&serial_probe);

    match config.camera.backend {
        CameraBackend::V4l2 => {
            let camera_path = config.camera.device.as_deref().unwrap_or("/dev/video0");
            let camera_probe = probe_video(Path::new(camera_path));
            print_probe(&camera_probe);
        }
        CameraBackend::Argus => {
            println!("camera probe: skipped direct device open for argus backend");
            println!(
                "camera note: validate this profile on the Jetson with GStreamer/Argus available"
            );
        }
    }

    println!();
    print_links("/dev/serial/by-id", "serial aliases");
    print_links("/dev/v4l/by-id", "camera aliases");

    Ok(())
}

fn probe_serial(path: &Path) -> DeviceProbe {
    probe_device(
        "servo bridge",
        path,
        libc::O_NOCTTY | libc::O_NONBLOCK | libc::O_CLOEXEC,
        Some("Permission denied usually means the user is missing the `dialout` group."),
    )
}

fn probe_video(path: &Path) -> DeviceProbe {
    probe_device(
        "camera",
        path,
        libc::O_NONBLOCK | libc::O_CLOEXEC,
        Some(
            "Permission denied usually means the user is missing the `video` group or a session ACL.",
        ),
    )
}

fn probe_device(
    label: &'static str,
    path: &Path,
    custom_flags: i32,
    hint: Option<&'static str>,
) -> DeviceProbe {
    let metadata = fs::metadata(path);
    let resolved_path = fs::canonicalize(path).ok();

    match metadata {
        Ok(metadata) => {
            let kind = if metadata.file_type().is_char_device() {
                "char-device".to_owned()
            } else if metadata.file_type().is_symlink() {
                "symlink".to_owned()
            } else {
                "other".to_owned()
            };

            let open_result = OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(custom_flags)
                .open(path)
                .map(|_| ());

            DeviceProbe {
                label,
                configured_path: path.to_path_buf(),
                resolved_path,
                kind,
                mode: metadata.mode() & 0o777,
                owner_uid: metadata.uid(),
                owner_gid: metadata.gid(),
                open_result,
                hint,
            }
        }
        Err(err) => DeviceProbe {
            label,
            configured_path: path.to_path_buf(),
            resolved_path,
            kind: "missing".to_owned(),
            mode: 0,
            owner_uid: 0,
            owner_gid: 0,
            open_result: Err(err),
            hint,
        },
    }
}

fn print_probe(probe: &DeviceProbe) {
    println!("{}:", probe.label);
    println!("  configured_path: {}", probe.configured_path.display());

    if let Some(resolved_path) = &probe.resolved_path {
        println!("  resolved_path: {}", resolved_path.display());
    }

    println!("  kind: {}", probe.kind);

    if probe.kind != "missing" {
        println!("  owner: {}:{}", probe.owner_uid, probe.owner_gid);
        println!("  mode: {:o}", probe.mode);
    }

    match &probe.open_result {
        Ok(()) => println!("  open_check: ok"),
        Err(err) => {
            println!("  open_check: {}", DisplayIo(err));
            if err.kind() == io::ErrorKind::PermissionDenied {
                if let Some(hint) = probe.hint {
                    println!("  hint: {}", hint);
                }
            }
        }
    }
}

fn print_links(path: &str, label: &str) {
    println!("{}:", label);

    match fs::read_dir(path) {
        Ok(entries) => {
            let mut found = false;

            for entry in entries.flatten() {
                found = true;
                let target = fs::canonicalize(entry.path())
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|_| "<unresolved>".to_owned());
                println!("  {} -> {}", entry.path().display(), target);
            }

            if !found {
                println!("  <none>");
            }
        }
        Err(err) => println!("  unavailable: {}", DisplayIo(&err)),
    }
}

struct DisplayIo<'a>(&'a io::Error);

impl fmt::Display for DisplayIo<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.raw_os_error() {
            Some(code) => write!(f, "{} (os error {})", self.0, code),
            None => write!(f, "{}", self.0),
        }
    }
}
