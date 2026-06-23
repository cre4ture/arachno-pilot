use std::{
    io::{self, BufRead, Write},
    time::Duration,
};

use arachno_msg::{TrajectoryEvent, TrajectoryFrame, TrajectoryHeader, TrajectoryRecord};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TrajectoryLogError {
    #[error("failed to write trajectory log: {0}")]
    Write(#[from] io::Error),
    #[error("failed to serialize trajectory record: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("failed to parse trajectory record on line {line}: {source}")]
    Parse {
        line: usize,
        source: serde_json::Error,
    },
    #[error(
        "trajectory frame time went backwards: previous frame at {previous_ms} ms, current frame at {current_ms} ms"
    )]
    NonMonotonicFrameTime { previous_ms: u64, current_ms: u64 },
}

pub struct TrajectoryLogWriter<W> {
    writer: W,
}

impl<W> TrajectoryLogWriter<W>
where
    W: Write,
{
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn write_header(&mut self, header: &TrajectoryHeader) -> Result<(), TrajectoryLogError> {
        self.write_record(&TrajectoryRecord::Header(header.clone()))
    }

    pub fn write_frame(&mut self, frame: &TrajectoryFrame) -> Result<(), TrajectoryLogError> {
        self.write_record(&TrajectoryRecord::Frame(frame.clone()))
    }

    pub fn write_event(&mut self, event: &TrajectoryEvent) -> Result<(), TrajectoryLogError> {
        self.write_record(&TrajectoryRecord::Event(event.clone()))
    }

    pub fn write_record(&mut self, record: &TrajectoryRecord) -> Result<(), TrajectoryLogError> {
        serde_json::to_writer(&mut self.writer, record)?;
        self.writer.write_all(b"\n")?;
        Ok(())
    }
}

pub fn read_trajectory_records<R>(reader: R) -> Result<Vec<TrajectoryRecord>, TrajectoryLogError>
where
    R: BufRead,
{
    let mut records = Vec::new();

    for (index, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let record = serde_json::from_str(&line).map_err(|source| TrajectoryLogError::Parse {
            line: index + 1,
            source,
        })?;
        records.push(record);
    }

    Ok(records)
}

pub fn replay_trajectory_records<S, F>(
    records: &[TrajectoryRecord],
    mut sleep: S,
    mut on_frame: F,
) -> Result<(), TrajectoryLogError>
where
    S: FnMut(Duration),
    F: FnMut(&TrajectoryFrame),
{
    let mut previous_elapsed_ms = None::<u64>;

    for record in records {
        let TrajectoryRecord::Frame(frame) = record else {
            continue;
        };

        if let Some(previous_elapsed_ms) = previous_elapsed_ms {
            if frame.elapsed_ms < previous_elapsed_ms {
                return Err(TrajectoryLogError::NonMonotonicFrameTime {
                    previous_ms: previous_elapsed_ms,
                    current_ms: frame.elapsed_ms,
                });
            }
            sleep(Duration::from_millis(
                frame.elapsed_ms - previous_elapsed_ms,
            ));
        }

        previous_elapsed_ms = Some(frame.elapsed_ms);
        on_frame(frame);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    use arachno_msg::{JointCommand, RobotSnapshot, ServoTelemetry};

    fn sample_header() -> TrajectoryHeader {
        TrajectoryHeader {
            format_version: 1,
            recorded_at_ms: 1_000,
            robot_name: "arachno-pilot".to_owned(),
            deployment_profile: "host-usb".to_owned(),
            control_hz: 150,
            command_hz: 20,
            config_path: Some("config/robot/default.toml".to_owned()),
        }
    }

    fn sample_frame(elapsed_ms: u64, servo_id: u8) -> TrajectoryFrame {
        TrajectoryFrame {
            elapsed_ms,
            snapshot: RobotSnapshot {
                timestamp_ms: 1_000 + elapsed_ms,
                body_mode: "stand_reference".to_owned(),
                telemetry: vec![ServoTelemetry {
                    servo_id,
                    present_position_ticks: 2_048,
                    ..ServoTelemetry::default()
                }],
                camera: None,
                imu: None,
            },
            commands: vec![JointCommand {
                servo_id,
                position_ticks: 2_100,
                speed_ticks: 200,
                acceleration: 10,
            }],
            motion_fault: None,
        }
    }

    #[test]
    fn trajectory_log_writer_and_reader_roundtrip_records() {
        let mut bytes = Vec::new();
        let mut writer = TrajectoryLogWriter::new(&mut bytes);
        writer
            .write_header(&sample_header())
            .expect("header should write");
        writer
            .write_event(&TrajectoryEvent {
                elapsed_ms: 0,
                kind: "mode_change".to_owned(),
                message: "telemetry -> stand".to_owned(),
            })
            .expect("event should write");
        writer
            .write_frame(&sample_frame(250, 11))
            .expect("frame should write");

        let records =
            read_trajectory_records(BufReader::new(Cursor::new(bytes))).expect("read should work");

        assert_eq!(records.len(), 3);
        assert!(matches!(records[0], TrajectoryRecord::Header(_)));
        assert!(matches!(records[1], TrajectoryRecord::Event(_)));
        match &records[2] {
            TrajectoryRecord::Frame(frame) => {
                assert_eq!(frame.elapsed_ms, 250);
                assert_eq!(frame.commands[0].servo_id, 11);
            }
            other => panic!("expected frame record, got {other:?}"),
        }
    }

    #[test]
    fn read_trajectory_records_reports_malformed_line_numbers() {
        let log = br#"{"record_type":"header","record":{"format_version":1,"recorded_at_ms":0,"robot_name":"a","deployment_profile":"b","control_hz":1,"command_hz":1,"config_path":null}}
not-json
"#;

        let err = read_trajectory_records(BufReader::new(Cursor::new(log.as_slice())))
            .expect_err("malformed line should fail");

        match err {
            TrajectoryLogError::Parse { line, .. } => assert_eq!(line, 2),
            other => panic!("expected parse error, got {other:?}"),
        }
    }

    #[test]
    fn replay_trajectory_records_uses_elapsed_time_deltas() {
        let records = vec![
            TrajectoryRecord::Header(sample_header()),
            TrajectoryRecord::Frame(sample_frame(100, 11)),
            TrajectoryRecord::Event(TrajectoryEvent {
                elapsed_ms: 120,
                kind: "note".to_owned(),
                message: "ignored during replay timing".to_owned(),
            }),
            TrajectoryRecord::Frame(sample_frame(350, 12)),
            TrajectoryRecord::Frame(sample_frame(500, 13)),
        ];

        let mut sleeps = Vec::new();
        let mut visited = Vec::new();
        replay_trajectory_records(
            &records,
            |duration| sleeps.push(duration),
            |frame| visited.push(frame.snapshot.telemetry[0].servo_id),
        )
        .expect("replay should succeed");

        assert_eq!(visited, vec![11, 12, 13]);
        assert_eq!(
            sleeps,
            vec![Duration::from_millis(250), Duration::from_millis(150)]
        );
    }

    #[test]
    fn replay_trajectory_records_rejects_non_monotonic_frame_times() {
        let records = vec![
            TrajectoryRecord::Frame(sample_frame(300, 11)),
            TrajectoryRecord::Frame(sample_frame(250, 12)),
        ];

        let err = replay_trajectory_records(&records, |_| {}, |_| {})
            .expect_err("non-monotonic replay should fail");

        match err {
            TrajectoryLogError::NonMonotonicFrameTime {
                previous_ms,
                current_ms,
            } => {
                assert_eq!(previous_ms, 300);
                assert_eq!(current_ms, 250);
            }
            other => panic!("expected non-monotonic time error, got {other:?}"),
        }
    }
}
