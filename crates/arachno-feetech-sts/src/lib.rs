use std::{
    collections::BTreeMap,
    io::{Read, Write},
    time::Duration,
};

use arachno_hal::{HalError, HalResult, ServoBus};
use arachno_msg::{JointCommand, ServoTelemetry};
use serialport::{ClearBuffer, SerialPort};

const ADDR_TORQUE_ENABLE: u8 = 40;
const ADDR_GOAL_POSITION: u8 = 42;
const ADDR_PRESENT_TELEMETRY: u8 = 56;
const PRESENT_TELEMETRY_LEN: u8 = 15;
const DEFAULT_TIMEOUT_MS: u64 = 20;

#[repr(u8)]
enum Instruction {
    Read = 0x02,
    Write = 0x03,
    SyncWrite = 0x83,
}

#[derive(Debug, Clone)]
struct StatusPacket {
    status: u8,
    params: Vec<u8>,
}

pub struct RealStsBus {
    servo_ids: Vec<u8>,
    port_path: String,
    baud_rate: u32,
    port: Box<dyn SerialPort>,
}

impl RealStsBus {
    pub fn open(
        port_path: impl Into<String>,
        baud_rate: u32,
        servo_ids: Vec<u8>,
    ) -> HalResult<Self> {
        let port_path = port_path.into();
        let port = serialport::new(&port_path, baud_rate)
            .timeout(Duration::from_millis(DEFAULT_TIMEOUT_MS))
            .open()
            .map_err(|err| {
                HalError::Communication(format!("failed to open {}: {err}", port_path))
            })?;

        Ok(Self {
            servo_ids,
            port_path,
            baud_rate,
            port,
        })
    }

    pub fn port_path(&self) -> &str {
        &self.port_path
    }

    pub fn baud_rate(&self) -> u32 {
        self.baud_rate
    }

    pub fn read_feedback_tolerant(&mut self, servo_id: u8) -> HalResult<ServoTelemetry> {
        let response =
            self.read_register_block(servo_id, ADDR_PRESENT_TELEMETRY, PRESENT_TELEMETRY_LEN)?;

        if response.params.len() < PRESENT_TELEMETRY_LEN as usize {
            return Err(HalError::Communication(format!(
                "servo {} telemetry response too short: expected {} bytes, got {}",
                servo_id,
                PRESENT_TELEMETRY_LEN,
                response.params.len()
            )));
        }

        let present_position_ticks = u16::from_le_bytes([response.params[0], response.params[1]]);
        let present_speed_ticks = i16::from_le_bytes([response.params[2], response.params[3]]);
        let present_load_raw = i16::from_le_bytes([response.params[4], response.params[5]]);
        let present_voltage_v = response.params[6] as f32 / 10.0;
        let present_temperature_c = response.params[7];
        let moving = response.params[10] != 0;
        let present_current_raw = u16::from_le_bytes([response.params[13], response.params[14]]);

        Ok(ServoTelemetry {
            servo_id,
            present_position_ticks,
            present_speed_ticks,
            present_load_pct: load_raw_to_pct(present_load_raw),
            present_voltage_v,
            present_current_ma: Some(raw_current_to_ma(present_current_raw)),
            present_temperature_c: Some(present_temperature_c),
            status_bits: Some(response.status),
            faults: decode_faults(response.status),
            moving,
        })
    }

    fn write_byte_no_response(&mut self, servo_id: u8, address: u8, value: u8) -> HalResult<()> {
        let packet = pack_instruction(servo_id, Instruction::Write, &[address, value]);
        let _ = self.port.clear(ClearBuffer::Input);
        self.port.write_all(&packet).map_err(|err| {
            HalError::Communication(format!("write to servo {} failed: {err}", servo_id))
        })?;
        self.port.flush().map_err(|err| {
            HalError::Communication(format!("flush to servo {} failed: {err}", servo_id))
        })?;
        let _ = self.port.clear(ClearBuffer::Input);
        Ok(())
    }

    fn read_register_block(
        &mut self,
        servo_id: u8,
        address: u8,
        len: u8,
    ) -> HalResult<StatusPacket> {
        let packet = pack_instruction(servo_id, Instruction::Read, &[address, len]);
        self.transfer(servo_id, &packet, len as usize + 6)
    }

    fn transfer(
        &mut self,
        servo_id: u8,
        packet: &[u8],
        response_len: usize,
    ) -> HalResult<StatusPacket> {
        let _ = self.port.clear(ClearBuffer::Input);
        self.port.write_all(packet).map_err(|err| {
            HalError::Communication(format!("write to servo {} failed: {err}", servo_id))
        })?;
        self.port.flush().map_err(|err| {
            HalError::Communication(format!("flush to servo {} failed: {err}", servo_id))
        })?;

        let mut response = vec![0u8; response_len];
        self.port.read_exact(&mut response).map_err(|err| {
            HalError::Communication(format!(
                "read from servo {} on {} failed: {}",
                servo_id, self.port_path, err
            ))
        })?;

        parse_status_packet(servo_id, &response)
    }
}

impl ServoBus for RealStsBus {
    fn servo_ids(&self) -> &[u8] {
        &self.servo_ids
    }

    fn enable_torque(&mut self, enabled: bool) -> HalResult<()> {
        for &servo_id in &self.servo_ids.clone() {
            self.write_byte_no_response(servo_id, ADDR_TORQUE_ENABLE, u8::from(enabled))?;
        }

        Ok(())
    }

    fn sync_write_positions(&mut self, commands: &[JointCommand]) -> HalResult<()> {
        for command in commands {
            if !self.servo_ids.contains(&command.servo_id) {
                return Err(HalError::DeviceUnavailable(format!(
                    "servo {} is not configured",
                    command.servo_id
                )));
            }
        }

        if commands.is_empty() {
            return Ok(());
        }

        let mut params = Vec::with_capacity(2 + commands.len() * 3);
        params.push(ADDR_GOAL_POSITION);
        params.push(2);

        for command in commands {
            let [low, high] = command.position_ticks.to_le_bytes();
            params.push(command.servo_id);
            params.push(low);
            params.push(high);
        }

        let packet = pack_instruction(0xFE, Instruction::SyncWrite, &params);
        self.port
            .write_all(&packet)
            .map_err(|err| HalError::Communication(format!("sync write failed: {err}")))?;
        self.port
            .flush()
            .map_err(|err| HalError::Communication(format!("sync write flush failed: {err}")))?;

        Ok(())
    }

    fn read_feedback(&mut self, servo_id: u8) -> HalResult<ServoTelemetry> {
        if !self.servo_ids.contains(&servo_id) {
            return Err(HalError::DeviceUnavailable(format!(
                "servo {} is not configured",
                servo_id
            )));
        }

        self.read_feedback_tolerant(servo_id)
    }
}

pub struct MockStsBus {
    servo_ids: Vec<u8>,
    last_commands: BTreeMap<u8, JointCommand>,
    torque_enabled: bool,
}

impl MockStsBus {
    pub fn new(servo_ids: Vec<u8>) -> Self {
        Self {
            servo_ids,
            last_commands: BTreeMap::new(),
            torque_enabled: false,
        }
    }

    pub fn integration_notes() -> &'static str {
        "Real STS serial support is available. Keep the mock for offline control testing and algorithm development."
    }
}

impl ServoBus for MockStsBus {
    fn servo_ids(&self) -> &[u8] {
        &self.servo_ids
    }

    fn enable_torque(&mut self, enabled: bool) -> HalResult<()> {
        self.torque_enabled = enabled;
        Ok(())
    }

    fn sync_write_positions(&mut self, commands: &[JointCommand]) -> HalResult<()> {
        for command in commands {
            if !self.servo_ids.contains(&command.servo_id) {
                return Err(HalError::DeviceUnavailable(format!(
                    "servo {} is not configured",
                    command.servo_id
                )));
            }

            self.last_commands.insert(command.servo_id, command.clone());
        }

        Ok(())
    }

    fn read_feedback(&mut self, servo_id: u8) -> HalResult<ServoTelemetry> {
        if !self.servo_ids.contains(&servo_id) {
            return Err(HalError::DeviceUnavailable(format!(
                "servo {} is not configured",
                servo_id
            )));
        }

        let command = self
            .last_commands
            .get(&servo_id)
            .cloned()
            .unwrap_or(JointCommand {
                servo_id,
                position_ticks: 2048,
                speed_ticks: 0,
                acceleration: 0,
            });

        Ok(ServoTelemetry {
            servo_id,
            present_position_ticks: command.position_ticks,
            present_speed_ticks: command.speed_ticks as i16,
            present_load_pct: if self.torque_enabled { 12.5 } else { 0.0 },
            present_voltage_v: 7.4,
            present_current_ma: if self.torque_enabled {
                Some(180)
            } else {
                Some(0)
            },
            present_temperature_c: Some(31),
            status_bits: Some(0),
            faults: Vec::new(),
            moving: self.torque_enabled && command.speed_ticks > 0,
        })
    }
}

fn calculate_checksum(id: u8, length: u8, instruction_or_status: u8, params: &[u8]) -> u8 {
    let mut sum: u32 = id as u32 + length as u32 + instruction_or_status as u32;
    for &param in params {
        sum += param as u32;
    }
    !(sum as u8)
}

fn pack_instruction(id: u8, instruction: Instruction, params: &[u8]) -> Vec<u8> {
    let length = (params.len() + 2) as u8;
    let instruction = instruction as u8;

    let mut packet = Vec::with_capacity(params.len() + 6);
    packet.extend_from_slice(&[0xFF, 0xFF, id, length, instruction]);
    packet.extend_from_slice(params);
    packet.push(calculate_checksum(id, length, instruction, params));
    packet
}

fn parse_status_packet(expected_id: u8, data: &[u8]) -> HalResult<StatusPacket> {
    if data.len() < 6 {
        return Err(HalError::Communication(format!(
            "response from servo {} too short: {} bytes",
            expected_id,
            data.len()
        )));
    }

    if data[0] != 0xFF || data[1] != 0xFF {
        return Err(HalError::Communication(format!(
            "invalid response header from servo {}: {:02X?}",
            expected_id, data
        )));
    }

    if data[2] != expected_id {
        return Err(HalError::Communication(format!(
            "response ID mismatch: expected {}, got {}",
            expected_id, data[2]
        )));
    }

    let length = data[3] as usize;
    if data.len() != length + 4 {
        return Err(HalError::Communication(format!(
            "response length mismatch from servo {}: header says {}, frame is {} bytes",
            expected_id,
            length,
            data.len()
        )));
    }

    let status = data[4];
    let params = data[5..data.len() - 1].to_vec();
    let actual_checksum = data[data.len() - 1];
    let expected_checksum = calculate_checksum(expected_id, data[3], status, &params);

    if actual_checksum != expected_checksum {
        return Err(HalError::Communication(format!(
            "checksum mismatch from servo {}: expected {:02X}, got {:02X}",
            expected_id, expected_checksum, actual_checksum
        )));
    }

    Ok(StatusPacket { status, params })
}

fn load_raw_to_pct(raw: i16) -> f32 {
    ((raw as i32).abs() as f32 / 10.0).min(100.0)
}

fn raw_current_to_ma(raw: u16) -> u16 {
    ((raw as f32) * 6.5).round() as u16
}

fn decode_faults(status: u8) -> Vec<String> {
    let mut faults = Vec::new();

    if status & 0x01 != 0 {
        faults.push("voltage".to_owned());
    }
    if status & 0x02 != 0 {
        faults.push("encoder".to_owned());
    }
    if status & 0x04 != 0 {
        faults.push("temperature".to_owned());
    }
    if status & 0x08 != 0 {
        faults.push("current".to_owned());
    }
    if status & 0x20 != 0 {
        faults.push("load".to_owned());
    }

    faults
}

#[cfg(test)]
mod tests {
    use super::{calculate_checksum, decode_faults, load_raw_to_pct, raw_current_to_ma};

    #[test]
    fn checksum_matches_expected_example() {
        let params = [56_u8, 15_u8];
        assert_eq!(calculate_checksum(13, 4, 0x02, &params), 0xA5);
    }

    #[test]
    fn decodes_fault_bits() {
        assert_eq!(
            decode_faults(0x2D),
            ["voltage", "temperature", "current", "load"]
        );
    }

    #[test]
    fn normalizes_raw_feedback_units() {
        assert_eq!(load_raw_to_pct(-125), 12.5);
        assert_eq!(raw_current_to_ma(100), 650);
    }
}
