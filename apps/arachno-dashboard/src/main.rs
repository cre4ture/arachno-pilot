use std::{
    collections::BTreeMap,
    fs,
    net::SocketAddr,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, RwLock},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use arachno_camera::RobotCamera;
use arachno_core::{CameraBackend, RobotConfig};
use arachno_feetech_sts::RealStsBus;
use arachno_hal::{CameraSource, ServoBus};
use arachno_msg::ServoTelemetry;
use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use clap::Parser;
use serde::Serialize;
use tokio::{net::TcpListener, process::Command};
use tokio_util::io::ReaderStream;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "config/robot/host-usb.toml")]
    config: PathBuf,
    #[arg(long, default_value = "127.0.0.1:3000")]
    listen: SocketAddr,
}

#[derive(Clone)]
struct AppState {
    config: RobotConfig,
    shared: Arc<RwLock<DashboardState>>,
}

#[derive(Debug, Clone, Serialize)]
struct DashboardState {
    robot_name: String,
    deployment_profile: String,
    compute_target: String,
    serial_port: String,
    camera_backend: CameraBackend,
    camera_device: Option<String>,
    camera_pipeline: String,
    updated_at_ms: u64,
    online_servo_count: usize,
    last_poll_error: Option<String>,
    servos: Vec<DashboardServoState>,
}

#[derive(Debug, Clone, Serialize)]
struct DashboardServoState {
    servo_id: u8,
    label: String,
    online: bool,
    error: Option<String>,
    telemetry: Option<ServoTelemetry>,
    position_deg: Option<f32>,
    position_percent: Option<f32>,
    speed_rpm: Option<f32>,
}

impl DashboardState {
    fn from_config(config: &RobotConfig) -> Self {
        let labels = servo_labels(config);
        let camera = RobotCamera::new(config.camera.clone());
        let mut servos = Vec::new();

        for servo_id in config.all_servo_ids() {
            let label = labels
                .get(&servo_id)
                .cloned()
                .unwrap_or_else(|| format!("servo-{servo_id}"));
            servos.push(DashboardServoState::offline(
                servo_id,
                label,
                "waiting for first poll",
            ));
        }

        Self {
            robot_name: config.robot.name.clone(),
            deployment_profile: config.deployment.profile.clone(),
            compute_target: config.deployment.compute.clone(),
            serial_port: config.bus.feetech.port.clone(),
            camera_backend: config.camera.backend,
            camera_device: config.camera.device.clone(),
            camera_pipeline: camera.pipeline_description().to_owned(),
            updated_at_ms: 0,
            online_servo_count: 0,
            last_poll_error: Some("poller not started yet".to_owned()),
            servos,
        }
    }
}

impl DashboardServoState {
    fn offline(servo_id: u8, label: String, message: impl Into<String>) -> Self {
        Self {
            servo_id,
            label,
            online: false,
            error: Some(message.into()),
            telemetry: None,
            position_deg: None,
            position_percent: None,
            speed_rpm: None,
        }
    }

    fn online(label: String, telemetry: ServoTelemetry) -> Self {
        let position_deg = Some(ticks_to_deg(telemetry.present_position_ticks));
        let position_percent = Some(telemetry.present_position_ticks as f32 / 4095.0 * 100.0);
        let speed_rpm = Some(speed_ticks_to_rpm(telemetry.present_speed_ticks));
        let error = if telemetry.faults.is_empty() {
            None
        } else {
            Some(telemetry.faults.join(", "))
        };

        Self {
            servo_id: telemetry.servo_id,
            label,
            online: true,
            error,
            telemetry: Some(telemetry),
            position_deg,
            position_percent,
            speed_rpm,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config_text = fs::read_to_string(&args.config)
        .with_context(|| format!("failed to read {}", args.config.display()))?;
    let config: RobotConfig = toml::from_str(&config_text)
        .with_context(|| format!("failed to parse {}", args.config.display()))?;

    let shared = Arc::new(RwLock::new(DashboardState::from_config(&config)));
    spawn_telemetry_worker(shared.clone(), config.clone());

    let app = Router::new()
        .route("/", get(index))
        .route("/api/state", get(api_state))
        .route("/camera.mjpg", get(camera_stream))
        .with_state(AppState { config, shared });

    let listener = TcpListener::bind(args.listen).await?;
    println!("dashboard: http://{}", args.listen);
    axum::serve(listener, app).await?;
    Ok(())
}

fn spawn_telemetry_worker(shared: Arc<RwLock<DashboardState>>, config: RobotConfig) {
    thread::spawn(move || {
        let labels = servo_labels(&config);
        let servo_ids = config.all_servo_ids();
        let mut bus = None::<RealStsBus>;

        loop {
            if bus.is_none() {
                match RealStsBus::open(
                    config.bus.feetech.port.clone(),
                    config.bus.feetech.baud_rate,
                    servo_ids.clone(),
                ) {
                    Ok(real_bus) => bus = Some(real_bus),
                    Err(err) => {
                        write_state(
                            &shared,
                            build_offline_state(
                                &config,
                                &labels,
                                &servo_ids,
                                format!("failed to open servo bus: {err}"),
                            ),
                        );
                        thread::sleep(Duration::from_millis(1000));
                        continue;
                    }
                }
            }

            let mut next_servos = Vec::with_capacity(servo_ids.len());
            let mut online_servo_count = 0;
            let mut should_reopen_bus = false;

            let Some(real_bus) = bus.as_mut() else {
                thread::sleep(Duration::from_millis(500));
                continue;
            };

            for servo_id in &servo_ids {
                let label = labels
                    .get(servo_id)
                    .cloned()
                    .unwrap_or_else(|| format!("servo-{servo_id}"));

                match real_bus.read_feedback(*servo_id) {
                    Ok(telemetry) => {
                        online_servo_count += 1;
                        next_servos.push(DashboardServoState::online(label, telemetry));
                    }
                    Err(err) => {
                        let message = err.to_string();
                        if message.contains("failed to open")
                            || message.contains("No such file")
                            || message.contains("Input/output error")
                        {
                            should_reopen_bus = true;
                        }

                        next_servos.push(DashboardServoState::offline(*servo_id, label, message));
                    }
                }
            }

            let last_poll_error = if online_servo_count == servo_ids.len() {
                None
            } else {
                Some(format!(
                    "{} of {} configured servos replied",
                    online_servo_count,
                    servo_ids.len()
                ))
            };

            write_state(
                &shared,
                DashboardState {
                    robot_name: config.robot.name.clone(),
                    deployment_profile: config.deployment.profile.clone(),
                    compute_target: config.deployment.compute.clone(),
                    serial_port: config.bus.feetech.port.clone(),
                    camera_backend: config.camera.backend,
                    camera_device: config.camera.device.clone(),
                    camera_pipeline: RobotCamera::new(config.camera.clone())
                        .pipeline_description()
                        .to_owned(),
                    updated_at_ms: now_ms(),
                    online_servo_count,
                    last_poll_error,
                    servos: next_servos,
                },
            );

            if should_reopen_bus {
                bus = None;
            }

            thread::sleep(Duration::from_millis(250));
        }
    });
}

fn build_offline_state(
    config: &RobotConfig,
    labels: &BTreeMap<u8, String>,
    servo_ids: &[u8],
    message: String,
) -> DashboardState {
    DashboardState {
        robot_name: config.robot.name.clone(),
        deployment_profile: config.deployment.profile.clone(),
        compute_target: config.deployment.compute.clone(),
        serial_port: config.bus.feetech.port.clone(),
        camera_backend: config.camera.backend,
        camera_device: config.camera.device.clone(),
        camera_pipeline: RobotCamera::new(config.camera.clone())
            .pipeline_description()
            .to_owned(),
        updated_at_ms: now_ms(),
        online_servo_count: 0,
        last_poll_error: Some(message.clone()),
        servos: servo_ids
            .iter()
            .map(|servo_id| {
                let label = labels
                    .get(servo_id)
                    .cloned()
                    .unwrap_or_else(|| format!("servo-{servo_id}"));
                DashboardServoState::offline(*servo_id, label, message.clone())
            })
            .collect(),
    }
}

fn write_state(shared: &Arc<RwLock<DashboardState>>, next: DashboardState) {
    if let Ok(mut state) = shared.write() {
        *state = next;
    }
}

async fn index() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

async fn api_state(State(state): State<AppState>) -> Result<Json<DashboardState>, StatusCode> {
    state
        .shared
        .read()
        .map(|snapshot| Json(snapshot.clone()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn camera_stream(State(state): State<AppState>) -> Response {
    if state.config.camera.backend != CameraBackend::V4l2 {
        return (
            StatusCode::NOT_IMPLEMENTED,
            "camera streaming is currently implemented for the host-usb v4l2 backend",
        )
            .into_response();
    }

    let Some(device) = state.config.camera.device.as_deref() else {
        return (StatusCode::BAD_REQUEST, "camera device missing from config").into_response();
    };

    let mut command = Command::new("ffmpeg");
    command
        .args(ffmpeg_camera_args(&state.config))
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to start ffmpeg for {}: {err}", device),
            )
                .into_response();
        }
    };

    let Some(stdout) = child.stdout.take() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "ffmpeg did not provide a stdout stream",
        )
            .into_response();
    };

    let stream = ReaderStream::new(stdout);
    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=ffmpeg",
        )
        .body(body)
        .unwrap_or_else(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to build response",
            )
                .into_response()
        })
}

fn ffmpeg_camera_args(config: &RobotConfig) -> Vec<String> {
    let mut args = vec![
        "-hide_banner".to_owned(),
        "-loglevel".to_owned(),
        "error".to_owned(),
        "-f".to_owned(),
        "video4linux2".to_owned(),
    ];

    let pixel_format = config.camera.pixel_format.to_ascii_lowercase();
    if pixel_format == "mjpg" || pixel_format == "mjpeg" {
        args.push("-input_format".to_owned());
        args.push("mjpeg".to_owned());
    } else {
        args.push("-input_format".to_owned());
        args.push(pixel_format);
    }

    args.push("-video_size".to_owned());
    args.push(format!("{}x{}", config.camera.width, config.camera.height));
    args.push("-framerate".to_owned());
    args.push(config.camera.fps.to_string());
    args.push("-i".to_owned());
    args.push(
        config
            .camera
            .device
            .clone()
            .unwrap_or_else(|| "/dev/video0".to_owned()),
    );
    args.push("-vf".to_owned());
    args.push(format!("fps={}", config.camera.fps.min(10)));
    args.push("-q:v".to_owned());
    args.push("7".to_owned());
    args.push("-f".to_owned());
    args.push("mpjpeg".to_owned());
    args.push("pipe:1".to_owned());
    args
}

fn servo_labels(config: &RobotConfig) -> BTreeMap<u8, String> {
    let mut labels = BTreeMap::new();
    for leg in &config.legs {
        labels.insert(leg.coxa_servo_id, format!("{} / coxa", leg.name));
        labels.insert(leg.femur_servo_id, format!("{} / femur", leg.name));
        labels.insert(leg.tibia_servo_id, format!("{} / tibia", leg.name));
    }
    labels
}

fn ticks_to_deg(ticks: u16) -> f32 {
    ticks as f32 * 360.0 / 4096.0
}

fn speed_ticks_to_rpm(speed_ticks: i16) -> f32 {
    speed_ticks as f32 * 60.0 / 4096.0
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

const DASHBOARD_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Arachno Dashboard</title>
  <style>
    :root {
      --bg: #0c1117;
      --panel: rgba(22, 29, 38, 0.86);
      --panel-strong: rgba(18, 24, 32, 0.96);
      --line: rgba(255, 255, 255, 0.1);
      --text: #eef3f7;
      --muted: #94a4b6;
      --accent: #ff9254;
      --accent-soft: rgba(255, 146, 84, 0.18);
      --ok: #65d6a4;
      --warn: #ffc26b;
      --bad: #ff6f61;
      --shadow: 0 18px 50px rgba(0, 0, 0, 0.34);
      --radius: 20px;
    }

    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
      color: var(--text);
      background:
        radial-gradient(circle at top left, rgba(255, 146, 84, 0.18), transparent 28rem),
        radial-gradient(circle at bottom right, rgba(70, 138, 255, 0.16), transparent 24rem),
        linear-gradient(160deg, #090c11 0%, #121a22 46%, #0c1117 100%);
      min-height: 100vh;
    }

    body::before {
      content: "";
      position: fixed;
      inset: 0;
      background-image:
        linear-gradient(rgba(255,255,255,0.03) 1px, transparent 1px),
        linear-gradient(90deg, rgba(255,255,255,0.03) 1px, transparent 1px);
      background-size: 28px 28px;
      mask-image: linear-gradient(to bottom, rgba(0,0,0,0.7), transparent);
      pointer-events: none;
    }

    .page {
      max-width: 1480px;
      margin: 0 auto;
      padding: 24px;
    }

    .hero {
      display: flex;
      justify-content: space-between;
      gap: 16px;
      align-items: end;
      margin-bottom: 18px;
    }

    .hero h1 {
      margin: 0;
      font-size: clamp(2rem, 3.6vw, 3.6rem);
      letter-spacing: -0.04em;
    }

    .subtitle {
      color: var(--muted);
      margin-top: 8px;
      max-width: 52rem;
    }

    .badge {
      display: inline-flex;
      align-items: center;
      gap: 10px;
      padding: 10px 14px;
      border-radius: 999px;
      background: rgba(0, 0, 0, 0.24);
      border: 1px solid var(--line);
      box-shadow: var(--shadow);
      color: var(--muted);
      font-size: 0.95rem;
    }

    .badge::before {
      content: "";
      width: 10px;
      height: 10px;
      border-radius: 999px;
      background: var(--warn);
      box-shadow: 0 0 0 0 rgba(255, 194, 107, 0.42);
      animation: pulse 1.6s infinite;
    }

    .badge.ok::before { background: var(--ok); box-shadow: 0 0 0 0 rgba(101, 214, 164, 0.42); }
    .badge.bad::before { background: var(--bad); box-shadow: 0 0 0 0 rgba(255, 111, 97, 0.42); }

    .layout {
      display: grid;
      grid-template-columns: minmax(20rem, 1.2fr) minmax(20rem, 0.8fr);
      gap: 18px;
    }

    .panel {
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: var(--radius);
      box-shadow: var(--shadow);
      backdrop-filter: blur(18px);
      overflow: hidden;
    }

    .panel-header {
      display: flex;
      justify-content: space-between;
      gap: 12px;
      align-items: center;
      padding: 18px 20px 0;
    }

    .panel-header h2 {
      margin: 0;
      font-size: 1.15rem;
      letter-spacing: 0.02em;
      text-transform: uppercase;
      color: var(--muted);
    }

    .panel-body {
      padding: 18px 20px 20px;
    }

    .stream-shell {
      background: linear-gradient(180deg, rgba(255,255,255,0.04), rgba(0,0,0,0.22));
      border-radius: 18px;
      border: 1px solid rgba(255,255,255,0.08);
      overflow: hidden;
      min-height: 18rem;
      display: flex;
      align-items: center;
      justify-content: center;
    }

    .stream-shell img {
      width: 100%;
      height: auto;
      display: block;
      background: #040608;
    }

    .stream-placeholder {
      color: var(--muted);
      padding: 22px;
      text-align: center;
      line-height: 1.5;
    }

    .stats {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 12px;
    }

    .stat {
      padding: 16px;
      background: var(--panel-strong);
      border-radius: 16px;
      border: 1px solid rgba(255,255,255,0.06);
    }

    .stat-label {
      color: var(--muted);
      font-size: 0.82rem;
      text-transform: uppercase;
      letter-spacing: 0.06em;
      margin-bottom: 8px;
    }

    .stat-value {
      font-size: 1.35rem;
      font-weight: 700;
      line-height: 1.1;
      word-break: break-word;
    }

    .stat-note {
      color: var(--muted);
      font-size: 0.92rem;
      margin-top: 8px;
      line-height: 1.4;
    }

    .servo-layout {
      margin-top: 18px;
      display: grid;
      gap: 12px;
    }

    .servo-map-shell {
      overflow-x: auto;
      padding-bottom: 6px;
    }

    .servo-map {
      position: relative;
      min-width: 980px;
      min-height: 760px;
      padding: 28px;
      border-radius: 22px;
      background:
        radial-gradient(circle at 50% 50%, rgba(255, 146, 84, 0.08), transparent 20rem),
        linear-gradient(180deg, rgba(255,255,255,0.03), rgba(0,0,0,0.22));
      border: 1px solid rgba(255,255,255,0.06);
    }

    .servo-orientation {
      color: var(--muted);
      font-size: 0.95rem;
      line-height: 1.5;
    }

    .axis-label {
      position: absolute;
      color: rgba(255,255,255,0.4);
      font-size: 0.78rem;
      letter-spacing: 0.18em;
      text-transform: uppercase;
    }

    .axis-front { top: 16px; left: 50%; transform: translateX(-50%); }
    .axis-rear { bottom: 16px; left: 50%; transform: translateX(-50%); }
    .axis-left {
      top: 50%;
      left: 16px;
      transform: translateY(-50%) rotate(-90deg);
      transform-origin: left top;
    }
    .axis-right {
      top: 50%;
      right: 16px;
      transform: translateY(-50%) rotate(90deg);
      transform-origin: right top;
    }

    .robot-body {
      position: absolute;
      inset: 50% auto auto 50%;
      transform: translate(-50%, -50%);
      width: 18rem;
      height: 25rem;
      display: grid;
      place-items: center;
      clip-path: polygon(18% 0%, 82% 0%, 100% 24%, 100% 76%, 82% 100%, 18% 100%, 0% 76%, 0% 24%);
      background:
        linear-gradient(160deg, rgba(255, 146, 84, 0.32), rgba(255, 146, 84, 0.08)),
        linear-gradient(180deg, rgba(255,255,255,0.08), rgba(0,0,0,0.24));
      border: 1px solid rgba(255,255,255,0.14);
      box-shadow:
        inset 0 1px 0 rgba(255,255,255,0.12),
        0 24px 44px rgba(0,0,0,0.28);
    }

    .robot-body::before {
      content: "";
      position: absolute;
      inset: 1.1rem;
      clip-path: inherit;
      background:
        radial-gradient(circle at top, rgba(255,255,255,0.08), transparent 58%),
        linear-gradient(180deg, rgba(0,0,0,0.1), rgba(0,0,0,0.3));
      border: 1px solid rgba(255,255,255,0.08);
    }

    .robot-body-core {
      position: relative;
      z-index: 1;
      display: flex;
      flex-direction: column;
      gap: 8px;
      align-items: center;
      text-align: center;
      padding: 0 1.4rem;
    }

    .robot-body-title {
      font-size: 1.5rem;
      font-weight: 700;
      letter-spacing: 0.02em;
    }

    .robot-body-note {
      color: rgba(255,255,255,0.68);
      font-size: 0.94rem;
      line-height: 1.45;
    }

    .leg-cluster {
      position: absolute;
      width: 20rem;
    }

    .leg-cluster::after {
      content: "";
      position: absolute;
      top: 50%;
      height: 2px;
      background: linear-gradient(90deg, rgba(255,255,255,0.08), rgba(255, 146, 84, 0.3));
    }

    .leg-cluster.left::after {
      right: -2.2rem;
      width: 2.2rem;
    }

    .leg-cluster.right::after {
      left: -2.2rem;
      width: 2.2rem;
      background: linear-gradient(90deg, rgba(255, 146, 84, 0.3), rgba(255,255,255,0.08));
    }

    .leg-cluster.front-left { top: 6%; left: 3%; }
    .leg-cluster.middle-left { top: 50%; left: 1.4%; transform: translateY(-50%); }
    .leg-cluster.rear-left { bottom: 6%; left: 3%; }
    .leg-cluster.front-right { top: 6%; right: 3%; }
    .leg-cluster.middle-right { top: 50%; right: 1.4%; transform: translateY(-50%); }
    .leg-cluster.rear-right { bottom: 6%; right: 3%; }

    .leg-name {
      margin-bottom: 10px;
      font-size: 0.78rem;
      letter-spacing: 0.16em;
      text-transform: uppercase;
      color: rgba(255,255,255,0.54);
    }

    .leg-chain {
      display: flex;
      gap: 10px;
      align-items: stretch;
    }

    .leg-chain.reverse {
      flex-direction: row-reverse;
    }

    .servo-node {
      flex: 1 1 0;
      min-width: 0;
      min-height: 166px;
      padding: 12px 10px;
      border-radius: 16px;
      background: linear-gradient(180deg, rgba(255,255,255,0.05), rgba(0,0,0,0.24));
      border: 1px solid rgba(255,255,255,0.08);
      box-shadow: inset 0 1px 0 rgba(255,255,255,0.04);
    }

    .servo-node.online { border-color: rgba(101, 214, 164, 0.25); }
    .servo-node.fault { border-color: rgba(255, 111, 97, 0.32); }
    .servo-node.offline {
      border-color: rgba(255,255,255,0.08);
      opacity: 0.78;
    }

    .servo-node-top {
      display: flex;
      justify-content: space-between;
      gap: 8px;
      align-items: start;
      margin-bottom: 10px;
    }

    .joint-name {
      font-size: 0.72rem;
      letter-spacing: 0.1em;
      text-transform: uppercase;
      color: var(--muted);
    }

    .servo-mini-state {
      padding: 5px 8px;
      border-radius: 999px;
      font-size: 0.72rem;
      background: rgba(255,255,255,0.06);
      color: rgba(255,255,255,0.7);
      white-space: nowrap;
    }

    .servo-node-id {
      font-size: 1.28rem;
      font-weight: 700;
      line-height: 1;
      margin-bottom: 6px;
    }

    .servo-node-pos {
      font-size: 0.96rem;
      color: rgba(255,255,255,0.84);
    }

    .servo-mini-grid {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 8px 10px;
      margin-top: 12px;
    }

    .servo-mini-grid strong {
      display: block;
      color: var(--muted);
      font-size: 0.68rem;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      margin-bottom: 2px;
    }

    .servo-mini-grid span {
      display: block;
      font-size: 0.84rem;
      color: rgba(255,255,255,0.86);
      line-height: 1.25;
    }

    .servo-node-error {
      margin-top: 12px;
      display: inline-flex;
      max-width: 100%;
      padding: 5px 8px;
      border-radius: 999px;
      font-size: 0.78rem;
      line-height: 1.25;
      background: rgba(255, 111, 97, 0.14);
      color: #ffb7ae;
      border: 1px solid rgba(255, 111, 97, 0.24);
    }

    .muted {
      color: var(--muted);
    }

    @media (max-width: 980px) {
      .layout { grid-template-columns: 1fr; }
      .stats { grid-template-columns: 1fr; }
      .page { padding: 18px; }
    }

    @keyframes pulse {
      0% { box-shadow: 0 0 0 0 currentColor; }
      70% { box-shadow: 0 0 0 10px transparent; }
      100% { box-shadow: 0 0 0 0 transparent; }
    }
  </style>
</head>
<body>
  <div class="page">
    <section class="hero">
      <div>
        <h1>Arachno Debug Dashboard</h1>
        <div class="subtitle">Live visibility into the tethered robot setup: servo feedback, fault states, and the current camera feed.</div>
      </div>
      <div id="status-badge" class="badge">waiting for telemetry</div>
    </section>

    <section class="layout">
      <div class="panel">
        <div class="panel-header">
          <h2>Camera</h2>
          <div class="muted" id="camera-meta">starting...</div>
        </div>
        <div class="panel-body">
          <div id="stream-shell" class="stream-shell">
            <div class="stream-placeholder" id="stream-placeholder">Preparing camera stream...</div>
            <img id="camera-stream" alt="Camera stream" hidden />
          </div>
        </div>
      </div>

      <div class="panel">
        <div class="panel-header">
          <h2>System State</h2>
          <div class="muted" id="updated-at">never</div>
        </div>
        <div class="panel-body">
          <div class="stats">
            <div class="stat">
              <div class="stat-label">Deployment</div>
              <div class="stat-value" id="deployment-profile">-</div>
              <div class="stat-note" id="compute-target">-</div>
            </div>
            <div class="stat">
              <div class="stat-label">Servo Replies</div>
              <div class="stat-value" id="servo-count">0 / 0</div>
              <div class="stat-note">Configured servos currently responding to feedback polls.</div>
            </div>
            <div class="stat">
              <div class="stat-label">Serial Bridge</div>
              <div class="stat-value" id="serial-port">-</div>
              <div class="stat-note" id="serial-note">Waiting for bus state.</div>
            </div>
            <div class="stat">
              <div class="stat-label">Camera Backend</div>
              <div class="stat-value" id="camera-backend">-</div>
              <div class="stat-note" id="camera-note">-</div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <section class="panel" style="margin-top: 18px;">
      <div class="panel-header">
        <h2>Servos</h2>
        <div class="muted" id="fault-summary">No servo data yet</div>
      </div>
      <div class="panel-body">
        <div class="servo-layout">
          <div class="servo-map-shell">
            <div class="servo-map">
              <div class="axis-label axis-front">Front</div>
              <div class="axis-label axis-left">Left</div>
              <div class="axis-label axis-right">Right</div>
              <div class="axis-label axis-rear">Rear</div>
              <div class="robot-body">
                <div class="robot-body-core">
                  <div class="robot-body-title">Hexapod Layout</div>
                  <div class="robot-body-note" id="robot-note">Coxa, femur, tibia are drawn from the body outward.</div>
                </div>
              </div>
              <div id="servo-map-legs"></div>
            </div>
          </div>
          <div class="servo-orientation">
            The map follows the robot's physical layout. Left legs are arms 1-3 from front to back, right legs are arms 4-6, and each leg is drawn from inside to outside as coxa, femur, tibia.
          </div>
        </div>
      </div>
    </section>
  </div>

  <script>
    const stateUrl = "/api/state";
    const cameraUrl = "/camera.mjpg";
    let streamStarted = false;
    const LEG_ORDER = [
      "front_left",
      "middle_left",
      "rear_left",
      "front_right",
      "middle_right",
      "rear_right",
    ];
    const LEG_META = {
      front_left: { label: "Front left", placement: "front-left left", side: "left" },
      middle_left: { label: "Middle left", placement: "middle-left left", side: "left" },
      rear_left: { label: "Rear left", placement: "rear-left left", side: "left" },
      front_right: { label: "Front right", placement: "front-right right", side: "right" },
      middle_right: { label: "Middle right", placement: "middle-right right", side: "right" },
      rear_right: { label: "Rear right", placement: "rear-right right", side: "right" },
    };
    const ARM_TO_LEG = {
      1: "front_left",
      2: "middle_left",
      3: "rear_left",
      4: "front_right",
      5: "middle_right",
      6: "rear_right",
    };
    const JOINT_LABEL = {
      1: "coxa",
      2: "femur",
      3: "tibia",
    };

    function fmt(value, digits = 1) {
      return Number.isFinite(value) ? value.toFixed(digits) : "n/a";
    }

    function legKeyForServo(servo) {
      return ARM_TO_LEG[Math.floor(servo.servo_id / 10)] ?? null;
    }

    function jointIndexForServo(servo) {
      return servo.servo_id % 10;
    }

    function compactError(message) {
      if (!message) return "";
      const lower = message.toLowerCase();
      if (lower.includes("timed out")) return "timeout";
      if (lower.includes("resource busy")) return "busy";
      if (lower.includes("failed to open")) return "bus open failed";
      return message.replace(/^communication failure:\s*/i, "");
    }

    function renderServoNode(servo) {
      const telemetry = servo.telemetry;
      const faults = telemetry?.faults ?? [];
      const classes = ["servo-node"];
      if (!servo.online) {
        classes.push("offline");
      } else if (faults.length) {
        classes.push("fault");
      } else {
        classes.push("online");
      }

      const jointIndex = jointIndexForServo(servo);
      const jointLabel = JOINT_LABEL[jointIndex] ?? "joint";
      const load = telemetry ? `${fmt(telemetry.present_load_pct)}%` : "n/a";
      const voltage = telemetry ? `${fmt(telemetry.present_voltage_v, 1)} V` : "n/a";
      const current = telemetry?.present_current_ma != null ? `${telemetry.present_current_ma} mA` : "n/a";
      const temp = telemetry?.present_temperature_c != null ? `${telemetry.present_temperature_c} °C` : "n/a";
      const stateLabel = telemetry ? (telemetry.moving ? "moving" : "ready") : "offline";
      const errorText = compactError(servo.error);

      return `
        <article class="${classes.join(" ")}">
          <div class="servo-node-top">
            <div>
              <div class="joint-name">${jointLabel}</div>
              <div class="servo-node-id">${servo.servo_id}</div>
              <div class="servo-node-pos">${servo.position_deg != null ? `${fmt(servo.position_deg, 1)}°` : "n/a"}</div>
            </div>
            <div class="servo-mini-state">${stateLabel}</div>
          </div>
          <div class="servo-mini-grid">
            <div><strong>Load</strong><span>${load}</span></div>
            <div><strong>Volt</strong><span>${voltage}</span></div>
            <div><strong>Temp</strong><span>${temp}</span></div>
            <div><strong>Current</strong><span>${current}</span></div>
          </div>
          ${errorText ? `<div class="servo-node-error">${errorText}</div>` : ""}
        </article>
      `;
    }

    function renderLegCluster(legKey, servos) {
      const meta = LEG_META[legKey];
      const sorted = [...servos].sort((left, right) => jointIndexForServo(left) - jointIndexForServo(right));
      const chainClass = meta.side === "left" ? "leg-chain reverse" : "leg-chain";

      return `
        <section class="leg-cluster ${meta.placement}">
          <div class="leg-name">${meta.label}</div>
          <div class="${chainClass}">
            ${sorted.map(renderServoNode).join("")}
          </div>
        </section>
      `;
    }

    function renderServoMap(servos) {
      const grouped = Object.fromEntries(LEG_ORDER.map((key) => [key, []]));
      for (const servo of servos) {
        const legKey = legKeyForServo(servo);
        if (legKey && grouped[legKey]) {
          grouped[legKey].push(servo);
        }
      }

      return LEG_ORDER.map((legKey) => renderLegCluster(legKey, grouped[legKey])).join("");
    }

    function updateBadge(ok, text) {
      const badge = document.getElementById("status-badge");
      badge.textContent = text;
      badge.classList.remove("ok", "bad");
      badge.classList.add(ok ? "ok" : "bad");
    }

    async function refresh() {
      try {
        const response = await fetch(stateUrl, { cache: "no-store" });
        if (!response.ok) throw new Error(`state fetch failed: ${response.status}`);
        const state = await response.json();

        document.getElementById("deployment-profile").textContent = state.deployment_profile;
        document.getElementById("compute-target").textContent = state.compute_target;
        document.getElementById("servo-count").textContent = `${state.online_servo_count} / ${state.servos.length}`;
        document.getElementById("serial-port").textContent = state.serial_port;
        document.getElementById("serial-note").textContent = state.last_poll_error ?? "All configured servos replied on the last poll.";
        document.getElementById("camera-backend").textContent = state.camera_backend;
        document.getElementById("camera-note").textContent = state.camera_device ?? state.camera_pipeline;
        document.getElementById("camera-meta").textContent = state.camera_pipeline;
        document.getElementById("updated-at").textContent = state.updated_at_ms ? new Date(state.updated_at_ms).toLocaleTimeString() : "never";

        const faulted = state.servos.filter((servo) => servo.telemetry && servo.telemetry.faults.length > 0).length;
        document.getElementById("fault-summary").textContent = `${faulted} servo(s) reporting status flags`;
        document.getElementById("robot-note").textContent =
          `${state.online_servo_count}/${state.servos.length} joints responding. Chains run from body outward as coxa, femur, tibia.`;
        document.getElementById("servo-map-legs").innerHTML = renderServoMap(state.servos);

        updateBadge(state.online_servo_count > 0, `${state.robot_name}: ${state.online_servo_count}/${state.servos.length} online`);

        if (state.camera_backend === "v4l2" && !streamStarted) {
          const img = document.getElementById("camera-stream");
          document.getElementById("stream-placeholder").hidden = true;
          img.hidden = false;
          img.src = cameraUrl;
          streamStarted = true;
        }

        if (state.camera_backend !== "v4l2") {
          document.getElementById("stream-placeholder").textContent =
            "This dashboard currently serves live video for the host-usb V4L2 camera path. The onboard Jetson profile is prepared, but its stream route still needs a Jetson-native capture backend.";
        }
      } catch (error) {
        updateBadge(false, "dashboard fetch error");
        document.getElementById("serial-note").textContent = String(error);
      }
    }

    refresh();
    setInterval(refresh, 500);
  </script>
</body>
</html>
"#;
