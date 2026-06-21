use std::{
    collections::BTreeSet,
    fmt,
    fs::{self, OpenOptions},
    io,
    os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

use anyhow::Context;
use arachno_camera::RobotCamera;
use arachno_core::{CameraBackend, RobotConfig};
use arachno_feetech_sts::{RealStsBus, STATUS_RETURN_LEVEL};
use arachno_hal::CameraSource;
use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "config/robot/default.toml")]
    config: PathBuf,
}

#[derive(Debug)]
struct DeviceProbe {
    label: String,
    configured_path: PathBuf,
    resolved_path: Option<PathBuf>,
    kind: String,
    mode: u32,
    owner_uid: u32,
    owner_gid: u32,
    open_result: Result<(), io::Error>,
    hint: Option<&'static str>,
}

#[derive(Debug)]
struct SerialBusProbe {
    device: DeviceProbe,
    leg_servo_probe: ServoIdProbe,
}

#[derive(Debug)]
enum ServoIdProbe {
    Complete {
        expected_ids: Vec<u8>,
        responding_ids: Vec<u8>,
        missing_ids: Vec<u8>,
    },
    Failed {
        expected_ids: Vec<u8>,
        error: String,
    },
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = RobotConfig::load_from_path(&args.config)
        .with_context(|| format!("failed to load {}", args.config.display()))?;
    let expected_leg_servo_ids = expected_leg_servo_ids(&config);

    println!("robot: {}", config.robot.name);
    println!("deployment_profile: {}", config.deployment.profile);
    println!("compute_target: {}", config.deployment.compute);
    println!("servo_legs_port: {}", config.bus.feetech.port);
    if !config.bus.feetech.additional_ports.is_empty() {
        println!(
            "servo_additional_ports: {}",
            config.bus.feetech.additional_ports.join(", ")
        );
    }
    println!(
        "expected_leg_servo_ids: {}",
        format_servo_ids(&expected_leg_servo_ids)
    );

    let camera = RobotCamera::new(config.camera.clone());
    println!("camera_backend: {:?}", config.camera.backend);
    println!("camera_pipeline: {}", camera.pipeline_description());
    println!();

    let serial_bus_probes = config
        .bus
        .feetech
        .configured_ports()
        .into_iter()
        .enumerate()
        .map(|(index, path)| probe_serial_bus(index, path, &config, &expected_leg_servo_ids))
        .collect::<Vec<_>>();
    let detected_legs_port = detect_legs_port_index(&serial_bus_probes);

    for (index, serial_probe) in serial_bus_probes.iter().enumerate() {
        print_serial_bus_probe(serial_probe, detected_legs_port == Some(index));
    }

    if let Some(index) = detected_legs_port {
        println!(
            "detected_legs_port: {}",
            serial_bus_probes[index].device.configured_path.display()
        );
        if index == 0 {
            println!("legs_port_configuration: matches the configured legs port");
        } else {
            println!(
                "legs_port_configuration: differs from the configured legs port; consider swapping it with additional port #{}",
                index
            );
        }
    } else {
        println!("detected_legs_port: inconclusive");
    }

    if !serial_bus_probes.is_empty() {
        println!();
    }

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

fn probe_serial_bus(
    index: usize,
    path: &str,
    config: &RobotConfig,
    expected_leg_servo_ids: &[u8],
) -> SerialBusProbe {
    let label = if index == 0 {
        "servo bridge (configured legs port)".to_owned()
    } else {
        format!("servo bridge (configured auxiliary port #{index})")
    };
    let device = probe_serial(label, Path::new(path));
    let leg_servo_probe =
        probe_leg_servo_ids(path, config.bus.feetech.baud_rate, expected_leg_servo_ids);

    SerialBusProbe {
        device,
        leg_servo_probe,
    }
}

fn probe_serial(label: impl Into<String>, path: &Path) -> DeviceProbe {
    probe_device(
        label.into(),
        path,
        libc::O_NOCTTY | libc::O_NONBLOCK | libc::O_CLOEXEC,
        Some("Permission denied usually means the user is missing the `dialout` group."),
    )
}

fn probe_video(path: &Path) -> DeviceProbe {
    probe_device(
        "camera".to_owned(),
        path,
        libc::O_NONBLOCK | libc::O_CLOEXEC,
        Some(
            "Permission denied usually means the user is missing the `video` group or a session ACL.",
        ),
    )
}

fn probe_device(
    label: String,
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

fn print_serial_bus_probe(probe: &SerialBusProbe, detected_legs_port: bool) {
    print_probe(&probe.device);

    match &probe.leg_servo_probe {
        ServoIdProbe::Complete {
            expected_ids,
            responding_ids,
            missing_ids,
        } => {
            println!(
                "  leg_servo_probe: {}/{} expected leg servos responded via register {}",
                responding_ids.len(),
                expected_ids.len(),
                STATUS_RETURN_LEVEL.address
            );
            println!(
                "  responding_leg_servo_ids: {}",
                format_servo_ids(responding_ids)
            );
            println!("  missing_leg_servo_ids: {}", format_servo_ids(missing_ids));
            println!(
                "  detected_role: {}",
                if detected_legs_port {
                    "legs port"
                } else {
                    "auxiliary servo port"
                }
            );
        }
        ServoIdProbe::Failed {
            expected_ids,
            error,
        } => {
            println!(
                "  leg_servo_probe: failed for {} expected leg servos",
                expected_ids.len()
            );
            println!("  detected_role: unknown");
            println!("  leg_servo_probe_error: {error}");
        }
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
            if err.kind() == io::ErrorKind::PermissionDenied
                && let Some(hint) = probe.hint
            {
                println!("  hint: {}", hint);
            }
        }
    }
}

fn expected_leg_servo_ids(config: &RobotConfig) -> Vec<u8> {
    config
        .all_servo_ids()
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn probe_leg_servo_ids(
    port_path: &str,
    baud_rate: u32,
    expected_leg_servo_ids: &[u8],
) -> ServoIdProbe {
    let expected_ids = expected_leg_servo_ids.to_vec();
    let mut bus = match RealStsBus::open(port_path.to_owned(), baud_rate, Vec::new()) {
        Ok(bus) => bus,
        Err(err) => {
            return ServoIdProbe::Failed {
                expected_ids,
                error: err.to_string(),
            };
        }
    };

    let mut responding_ids = Vec::new();
    let mut missing_ids = Vec::new();

    for &servo_id in expected_leg_servo_ids {
        match bus.read_register_u8(servo_id, STATUS_RETURN_LEVEL.address) {
            Ok(_) => responding_ids.push(servo_id),
            Err(_) => missing_ids.push(servo_id),
        }
    }

    ServoIdProbe::Complete {
        expected_ids,
        responding_ids,
        missing_ids,
    }
}

fn detect_legs_port_index(serial_bus_probes: &[SerialBusProbe]) -> Option<usize> {
    let mut best_index = None;
    let mut best_count = 0usize;
    let mut saw_tie = false;

    for (index, probe) in serial_bus_probes.iter().enumerate() {
        let response_count = match &probe.leg_servo_probe {
            ServoIdProbe::Complete { responding_ids, .. } => responding_ids.len(),
            ServoIdProbe::Failed { .. } => 0,
        };

        if response_count == 0 {
            continue;
        }

        if response_count > best_count {
            best_index = Some(index);
            best_count = response_count;
            saw_tie = false;
        } else if response_count == best_count {
            saw_tie = true;
        }
    }

    if saw_tie { None } else { best_index }
}

fn format_servo_ids(ids: &[u8]) -> String {
    if ids.is_empty() {
        return "<none>".to_owned();
    }

    ids.iter().map(u8::to_string).collect::<Vec<_>>().join(", ")
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

#[cfg(test)]
mod tests {
    use super::{
        DeviceProbe, SerialBusProbe, ServoIdProbe, detect_legs_port_index, expected_leg_servo_ids,
    };
    use arachno_core::{
        BusConfig, CameraBackend, CameraConfig, DeploymentConfig, FeetechBusConfig, LearningConfig,
        LegConfig, LocomotionConfig, RobotConfig, RobotMeta, SafetyConfig, SemanticPoseSet,
        ServoEepromConfig,
    };
    use std::path::PathBuf;

    #[test]
    fn detect_legs_port_prefers_unique_highest_match_count() {
        let probes = vec![
            serial_bus_probe(vec![11, 12], vec![13]),
            serial_bus_probe(vec![11, 12, 13], Vec::new()),
            serial_bus_probe(Vec::new(), vec![11, 12, 13]),
        ];

        assert_eq!(detect_legs_port_index(&probes), Some(1));
    }

    #[test]
    fn detect_legs_port_is_inconclusive_on_tie() {
        let probes = vec![
            serial_bus_probe(vec![11, 12], vec![13]),
            serial_bus_probe(vec![21, 22], vec![23]),
        ];

        assert_eq!(detect_legs_port_index(&probes), None);
    }

    #[test]
    fn expected_leg_servo_ids_are_sorted_and_unique() {
        let config = RobotConfig {
            deployment: DeploymentConfig {
                profile: "test".to_owned(),
                compute: "linux".to_owned(),
            },
            robot: RobotMeta {
                name: "test".to_owned(),
                control_hz: 100,
                perception_hz: 20,
            },
            servo_store: None,
            pose_store: None,
            workspace_store: None,
            semantic_calibration_store: None,
            poses: SemanticPoseSet::default(),
            leg_workspaces: Default::default(),
            servo_eeprom: ServoEepromConfig::default(),
            bus: BusConfig {
                feetech: FeetechBusConfig::default(),
            },
            camera: CameraConfig {
                name: "camera".to_owned(),
                backend: CameraBackend::V4l2,
                device: None,
                sensor_id: None,
                width: 640,
                height: 480,
                fps: 30,
                fov_deg: 60.0,
                pixel_format: "MJPG".to_owned(),
            },
            imu: None,
            safety: SafetyConfig::default(),
            learning: LearningConfig {
                mode: "shadow".to_owned(),
                policy_transport: "unix-socket".to_owned(),
                policy_path: "policy.onnx".to_owned(),
            },
            locomotion: LocomotionConfig::default(),
            legs: vec![
                LegConfig {
                    name: "front_left".to_owned(),
                    coxa_servo_id: 21,
                    femur_servo_id: 22,
                    tibia_servo_id: 23,
                    coxa_stand_reference_ticks: None,
                    femur_stand_reference_ticks: None,
                    tibia_stand_reference_ticks: None,
                    coxa_zero_reference_ticks: None,
                    femur_zero_reference_ticks: None,
                    tibia_zero_reference_ticks: None,
                    coxa_lay_down_ticks: None,
                    femur_lay_down_ticks: None,
                    tibia_lay_down_ticks: None,
                    coxa_forward_sign: 1,
                    femur_lift_sign: 1,
                    tibia_lift_sign: 1,
                    coxa_zero_heading_deg: None,
                    coxa_length_cm: Some(1.0),
                    femur_length_cm: Some(1.0),
                    tibia_length_cm: Some(1.0),
                },
                LegConfig {
                    name: "front_right".to_owned(),
                    coxa_servo_id: 21,
                    femur_servo_id: 12,
                    tibia_servo_id: 13,
                    coxa_stand_reference_ticks: None,
                    femur_stand_reference_ticks: None,
                    tibia_stand_reference_ticks: None,
                    coxa_zero_reference_ticks: None,
                    femur_zero_reference_ticks: None,
                    tibia_zero_reference_ticks: None,
                    coxa_lay_down_ticks: None,
                    femur_lay_down_ticks: None,
                    tibia_lay_down_ticks: None,
                    coxa_forward_sign: 1,
                    femur_lift_sign: 1,
                    tibia_lift_sign: 1,
                    coxa_zero_heading_deg: None,
                    coxa_length_cm: Some(1.0),
                    femur_length_cm: Some(1.0),
                    tibia_length_cm: Some(1.0),
                },
            ],
        };

        assert_eq!(expected_leg_servo_ids(&config), vec![12, 13, 21, 22, 23]);
    }

    fn serial_bus_probe(responding_ids: Vec<u8>, missing_ids: Vec<u8>) -> SerialBusProbe {
        SerialBusProbe {
            device: DeviceProbe {
                label: "test".to_owned(),
                configured_path: PathBuf::from("/dev/null"),
                resolved_path: None,
                kind: "char-device".to_owned(),
                mode: 0o660,
                owner_uid: 0,
                owner_gid: 0,
                open_result: Ok(()),
                hint: None,
            },
            leg_servo_probe: ServoIdProbe::Complete {
                expected_ids: responding_ids
                    .iter()
                    .chain(missing_ids.iter())
                    .copied()
                    .collect(),
                responding_ids,
                missing_ids,
            },
        }
    }
}
