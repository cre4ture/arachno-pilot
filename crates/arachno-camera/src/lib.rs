use arachno_core::{CameraBackend, CameraConfig};
use arachno_hal::{CameraSource, HalResult};
use arachno_msg::CameraFrameMeta;

pub struct RobotCamera {
    config: CameraConfig,
    pipeline: String,
    frame_index: u64,
}

impl RobotCamera {
    pub fn new(config: CameraConfig) -> Self {
        let pipeline = match config.backend {
            CameraBackend::Argus => format!(
                "nvarguscamerasrc sensor-id={} ! video/x-raw(memory:NVMM), width=(int){}, height=(int){}, framerate=(fraction){}/1, format=(string){} ! nvvidconv ! video/x-raw, format=(string)BGRx ! videoconvert ! video/x-raw, format=(string)BGR ! appsink max-buffers=1 sync=false",
                config.sensor_id.unwrap_or(0),
                config.width,
                config.height,
                config.fps,
                config.pixel_format,
            ),
            CameraBackend::V4l2 => v4l2_pipeline(&config),
        };

        Self {
            config,
            pipeline,
            frame_index: 0,
        }
    }
}

fn v4l2_pipeline(config: &CameraConfig) -> String {
    let device = config.device.as_deref().unwrap_or("/dev/video0");
    let pixel_format = config.pixel_format.to_ascii_uppercase();

    if pixel_format == "MJPG" || pixel_format == "JPEG" {
        format!(
            "v4l2src device={} ! image/jpeg, width=(int){}, height=(int){}, framerate=(fraction){}/1 ! jpegdec ! videoconvert ! video/x-raw, format=(string)BGR ! appsink max-buffers=1 sync=false",
            device, config.width, config.height, config.fps,
        )
    } else {
        format!(
            "v4l2src device={} ! video/x-raw, width=(int){}, height=(int){}, framerate=(fraction){}/1, format=(string){} ! videoconvert ! video/x-raw, format=(string)BGR ! appsink max-buffers=1 sync=false",
            device, config.width, config.height, config.fps, config.pixel_format,
        )
    }
}

impl CameraSource for RobotCamera {
    fn start(&mut self) -> HalResult<()> {
        Ok(())
    }

    fn next_frame(&mut self) -> HalResult<Option<CameraFrameMeta>> {
        self.frame_index += 1;

        Ok(Some(CameraFrameMeta {
            frame_index: self.frame_index,
            width: self.config.width,
            height: self.config.height,
            format: "BGR".to_owned(),
        }))
    }

    fn pipeline_description(&self) -> &str {
        &self.pipeline
    }
}
