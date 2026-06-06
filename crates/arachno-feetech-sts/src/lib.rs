use std::{
    collections::BTreeMap,
    io::{ErrorKind, Read, Write},
    thread,
    time::Duration,
};

use arachno_core::{ServoEepromEntry, ServoRegisterWidth};
use arachno_hal::{HalError, HalResult, ServoBus, sync_current_pose};
use arachno_msg::{JointCommand, ServoTelemetry};
pub use registers::{
    GOAL_POSITION, KNOWN_REGISTERS, LOCK_MARK, MAX_TORQUE_LIMIT, PRESENT_TELEMETRY, RegisterAccess,
    RegisterArea, STATUS_RETURN_LEVEL, ServoRegister, TORQUE_ENABLE, TORQUE_LIMIT, lookup_register,
};
use registers::{RegisterAccess::ReadWrite, RegisterArea::Eeprom};
use serialport::{ClearBuffer, SerialPort};
use tracing::trace;

mod registers;

const DEFAULT_TIMEOUT_MS: u64 = 50;
const WRITE_STATUS_PACKET_LEN: usize = 6;
const VERIFIED_TORQUE_LIMIT_VERIFY_ATTEMPTS: usize = 6;
const VERIFIED_TORQUE_LIMIT_READBACK_ATTEMPTS: usize = 3;
const VERIFIED_TORQUE_LIMIT_VERIFY_SLEEP_MS: u64 = 30;
const VERIFIED_TORQUE_LIMIT_APPLY_SLEEP_MS: u64 = 20;
const VERIFIED_TORQUE_LIMIT_PHASE_SETTLE_MS: u64 = 250;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteConfirmationMode {
    Required,
    Optional,
}

pub struct RealStsBus {
    servo_ids: Vec<u8>,
    port_path: String,
    baud_rate: u32,
    write_confirmation_mode: WriteConfirmationMode,
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

        let bus = Self {
            servo_ids,
            port_path,
            baud_rate,
            write_confirmation_mode: WriteConfirmationMode::Required,
            port,
        };
        trace!(
            target: "arachno_feetech_sts::bus",
            port = %bus.port_path,
            baud_rate = bus.baud_rate,
            configured_servos = ?bus.servo_ids,
            "opened serial bus"
        );
        Ok(bus)
    }

    pub fn port_path(&self) -> &str {
        &self.port_path
    }

    pub fn baud_rate(&self) -> u32 {
        self.baud_rate
    }

    pub fn write_confirmation_mode(&self) -> WriteConfirmationMode {
        self.write_confirmation_mode
    }

    pub fn set_write_confirmation_mode(
        &mut self,
        mode: WriteConfirmationMode,
    ) -> WriteConfirmationMode {
        let previous = self.write_confirmation_mode;
        self.write_confirmation_mode = mode;
        previous
    }

    pub fn read_feedback_tolerant(&mut self, servo_id: u8) -> HalResult<ServoTelemetry> {
        let response = self.read_register_block(
            servo_id,
            PRESENT_TELEMETRY.address,
            PRESENT_TELEMETRY.width_bytes,
        )?;

        if response.params.len() < PRESENT_TELEMETRY.width_bytes as usize {
            return Err(HalError::Communication(format!(
                "servo {} telemetry response too short: expected {} bytes, got {}",
                servo_id,
                PRESENT_TELEMETRY.width_bytes,
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

        let telemetry = ServoTelemetry {
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
        };

        trace!(
            target: "arachno_feetech_sts::bus",
            servo_id = telemetry.servo_id,
            position_ticks = telemetry.present_position_ticks,
            speed_ticks = telemetry.present_speed_ticks,
            load_pct = telemetry.present_load_pct,
            voltage_v = telemetry.present_voltage_v,
            temperature_c = telemetry.present_temperature_c.unwrap_or_default(),
            moving = telemetry.moving,
            faults = ?telemetry.faults,
            "feedback"
        );

        Ok(telemetry)
    }

    fn write_register_with_confirmation(
        &mut self,
        servo_id: u8,
        register: &'static ServoRegister,
        params: &[u8],
        scope: &str,
    ) -> HalResult<()> {
        let mut instruction_params = Vec::with_capacity(1 + params.len());
        instruction_params.push(register.address);
        instruction_params.extend_from_slice(params);
        let packet = pack_instruction(servo_id, Instruction::Write, &instruction_params);
        trace!(
            target: "arachno_feetech_sts::bus",
            servo_id,
            register = register.name,
            address = register.address,
            scope,
            packet = %format_bytes_hex(&packet),
            "tx write-register"
        );
        self.transmit_packet(servo_id, &packet)?;
        match self.write_confirmation_mode {
            WriteConfirmationMode::Required => {
                let response =
                    self.receive_status_packet_required(servo_id, WRITE_STATUS_PACKET_LEN)?;
                self.validate_write_confirmation(servo_id, register, scope, &response)?;
            }
            WriteConfirmationMode::Optional => {
                if let Some(response) =
                    self.receive_status_packet_optional(servo_id, WRITE_STATUS_PACKET_LEN)?
                {
                    self.validate_write_confirmation(servo_id, register, scope, &response)?;
                } else {
                    trace!(
                        target: "arachno_feetech_sts::bus",
                        servo_id,
                        register = register.name,
                        address = register.address,
                        scope,
                        "rx-skip optional write-confirmation not received"
                    );
                }
            }
        }
        Ok(())
    }

    fn write_runtime_byte(&mut self, servo_id: u8, address: u8, value: u8) -> HalResult<()> {
        let register = runtime_writable_register(address, 1)?;
        self.write_register_with_confirmation(servo_id, register, &[value], "runtime")
    }

    fn write_runtime_word(&mut self, servo_id: u8, address: u8, value: u16) -> HalResult<()> {
        let register = runtime_writable_register(address, 2)?;
        self.write_register_with_confirmation(servo_id, register, &value.to_le_bytes(), "runtime")
    }

    pub fn write_persistent_register_u8(
        &mut self,
        servo_id: u8,
        address: u8,
        value: u8,
    ) -> HalResult<()> {
        let register = persistent_writable_register(address, 1)?;
        self.write_register_with_confirmation(servo_id, register, &[value], "persistent")
    }

    pub fn write_persistent_register_u16(
        &mut self,
        servo_id: u8,
        address: u8,
        value: u16,
    ) -> HalResult<()> {
        let register = persistent_writable_register(address, 2)?;
        self.write_register_with_confirmation(
            servo_id,
            register,
            &value.to_le_bytes(),
            "persistent",
        )
    }

    pub fn read_register_u8(&mut self, servo_id: u8, address: u8) -> HalResult<u8> {
        let response = self.read_register_block(servo_id, address, 1)?;
        response.params.first().copied().ok_or_else(|| {
            HalError::Communication(format!(
                "servo {} register {} response too short: expected 1 byte, got {}",
                servo_id,
                address,
                response.params.len()
            ))
        })
    }

    pub fn read_register_u16(&mut self, servo_id: u8, address: u8) -> HalResult<u16> {
        let response = self.read_register_block(servo_id, address, 2)?;
        if response.params.len() < 2 {
            return Err(HalError::Communication(format!(
                "servo {} register {} response too short: expected 2 bytes, got {}",
                servo_id,
                address,
                response.params.len()
            )));
        }

        Ok(u16::from_le_bytes([response.params[0], response.params[1]]))
    }

    // Attention: use with caution as changing torque limits while the robot is in the air can cause sudden motion when the torque limit takes effect.
    // It's best to call this right after setting the desired position for all servos with sync_write_positions, and avoid calling it while the robot is in the air.
    pub fn set_servo_torque_limit(&mut self, servo_id: u8, torque_limit: u16) -> HalResult<()> {
        self.write_runtime_word(servo_id, TORQUE_LIMIT.address, torque_limit)
    }

    pub fn read_servo_torque_limit(&mut self, servo_id: u8) -> HalResult<u16> {
        self.read_register_u16(servo_id, TORQUE_LIMIT.address)
    }

    pub fn read_eeprom_write_lock(&mut self, servo_id: u8) -> HalResult<bool> {
        Ok(self.read_register_u8(servo_id, LOCK_MARK.address)? != 0)
    }

    pub fn set_eeprom_write_lock(&mut self, servo_id: u8, locked: bool) -> HalResult<()> {
        self.write_runtime_byte(servo_id, LOCK_MARK.address, u8::from(locked))?;
        let observed = self.read_eeprom_write_lock(servo_id)?;
        if observed != locked {
            return Err(HalError::Communication(format!(
                "failed to set EEPROM write lock on servo {}: expected {}, observed {}",
                servo_id,
                u8::from(locked),
                u8::from(observed)
            )));
        }
        Ok(())
    }

    fn read_register_block(
        &mut self,
        servo_id: u8,
        address: u8,
        len: u8,
    ) -> HalResult<StatusPacket> {
        let packet = pack_instruction(servo_id, Instruction::Read, &[address, len]);
        trace!(
            target: "arachno_feetech_sts::bus",
            servo_id,
            address,
            len,
            packet = %format_bytes_hex(&packet),
            "tx read"
        );
        self.transfer(servo_id, &packet, len as usize + 6)
    }

    fn transfer(
        &mut self,
        servo_id: u8,
        packet: &[u8],
        response_len: usize,
    ) -> HalResult<StatusPacket> {
        self.transmit_packet(servo_id, packet)?;
        self.receive_status_packet_required(servo_id, response_len)
    }

    fn transmit_packet(&mut self, servo_id: u8, packet: &[u8]) -> HalResult<()> {
        let _ = self.port.clear(ClearBuffer::Input);
        if let Err(err) = self.port.write_all(packet) {
            trace!(
                target: "arachno_feetech_sts::bus",
                servo_id,
                packet = %format_bytes_hex(packet),
                error = %err,
                "tx-error"
            );
            return Err(HalError::Communication(format!(
                "write to servo {} failed: {err}",
                servo_id
            )));
        }
        if let Err(err) = self.port.flush() {
            trace!(
                target: "arachno_feetech_sts::bus",
                servo_id,
                error = %err,
                "tx-flush-error"
            );
            return Err(HalError::Communication(format!(
                "flush to servo {} failed: {err}",
                servo_id
            )));
        }
        Ok(())
    }

    fn receive_status_packet_required(
        &mut self,
        servo_id: u8,
        response_len: usize,
    ) -> HalResult<StatusPacket> {
        let response = self
            .read_response_frame(servo_id, response_len)
            .map_err(|err| {
                HalError::Communication(format!(
                    "read from servo {} on {} failed: {}",
                    servo_id, self.port_path, err
                ))
            })?;
        self.parse_received_status_packet(servo_id, &response)
    }

    fn receive_status_packet_optional(
        &mut self,
        servo_id: u8,
        response_len: usize,
    ) -> HalResult<Option<StatusPacket>> {
        match self.read_response_frame(servo_id, response_len) {
            Ok(response) => self
                .parse_received_status_packet(servo_id, &response)
                .map(Some),
            Err(err)
                if err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock =>
            {
                let _ = self.port.clear(ClearBuffer::Input);
                trace!(
                    target: "arachno_feetech_sts::bus",
                    servo_id,
                    expected_len = response_len,
                    "rx-timeout optional"
                );
                Ok(None)
            }
            Err(err) => Err(HalError::Communication(format!(
                "read from servo {} on {} failed: {}",
                servo_id, self.port_path, err
            ))),
        }
    }

    fn read_response_frame(
        &mut self,
        servo_id: u8,
        response_len: usize,
    ) -> std::io::Result<Vec<u8>> {
        let mut response = vec![0u8; response_len];
        self.port.read_exact(&mut response).map_err(|err| {
            trace!(
                target: "arachno_feetech_sts::bus",
                servo_id,
                expected_len = response_len,
                error = %err,
                "rx-error"
            );
            err
        })?;
        Ok(response)
    }

    fn parse_received_status_packet(
        &self,
        servo_id: u8,
        response: &[u8],
    ) -> HalResult<StatusPacket> {
        trace!(
            target: "arachno_feetech_sts::bus",
            servo_id,
            packet = %format_bytes_hex(response),
            "rx"
        );
        parse_status_packet(servo_id, response)
    }

    fn validate_write_confirmation(
        &self,
        servo_id: u8,
        register: &ServoRegister,
        scope: &str,
        response: &StatusPacket,
    ) -> HalResult<()> {
        if !response.params.is_empty() {
            trace!(
                target: "arachno_feetech_sts::bus",
                servo_id,
                register = register.name,
                address = register.address,
                scope,
                params = %format_bytes_hex(&response.params),
                "rx-invalid write-confirmation-params"
            );
            return Err(HalError::Communication(format!(
                "write confirmation from servo {} for {} unexpectedly returned {} parameter byte(s)",
                servo_id,
                register.name,
                response.params.len()
            )));
        }

        if response.status != 0 {
            let faults = decode_faults(response.status);
            trace!(
                target: "arachno_feetech_sts::bus",
                servo_id,
                register = register.name,
                address = register.address,
                scope,
                status = response.status,
                faults = ?faults,
                "rx-fault write-confirmation"
            );
            return Err(HalError::Communication(format!(
                "write confirmation from servo {} for {} reported status 0x{:02X} ({})",
                servo_id,
                register.name,
                response.status,
                if faults.is_empty() {
                    "unknown fault".to_owned()
                } else {
                    faults.join(", ")
                }
            )));
        }

        trace!(
            target: "arachno_feetech_sts::bus",
            servo_id,
            register = register.name,
            address = register.address,
            scope,
            status = response.status,
            "rx-ok write-confirmation"
        );

        Ok(())
    }

    fn check_servo_ids(&self, servo_ids: &[u8]) -> HalResult<()> {
        for &servo_id in servo_ids {
            if !self.servo_ids.contains(&servo_id) {
                return Err(HalError::DeviceUnavailable(format!(
                    "servo {} is not configured",
                    servo_id
                )));
            }
        }
        Ok(())
    }

    pub fn enable_torque_on_id(&mut self, servo_id: u8, enabled: bool) -> HalResult<()> {
        self.check_servo_ids(&[servo_id])?;
        self.write_runtime_byte(servo_id, TORQUE_ENABLE.address, u8::from(enabled))
    }

    pub fn enable_torque_on_ids(&mut self, servo_ids: &[u8], enabled: bool) -> HalResult<()> {
        for &servo_id in servo_ids {
            self.enable_torque_on_id(servo_id, enabled)?;
        }

        Ok(())
    }
}

pub fn validate_servo_eeprom_profile(
    bus: &mut RealStsBus,
    servo_ids: &[u8],
    entries: &[ServoEepromEntry],
) -> HalResult<()> {
    for entry in entries {
        for &servo_id in servo_ids {
            validate_servo_eeprom_entry(bus, servo_id, entry)?;
        }
    }

    Ok(())
}

pub fn validate_servo_eeprom_entry_value(
    bus: &mut RealStsBus,
    servo_id: u8,
    entry: &ServoEepromEntry,
) -> HalResult<u16> {
    let observed = match entry.width {
        ServoRegisterWidth::U8 => u16::from(bus.read_register_u8(servo_id, entry.address)?),
        ServoRegisterWidth::U16 => bus.read_register_u16(servo_id, entry.address)?,
    };

    let expected = match entry.width {
        ServoRegisterWidth::U8 => u16::from(u8::try_from(entry.value).map_err(|_| {
            HalError::Unsupported(format!(
                "EEPROM entry {} value {} does not fit into u8",
                entry.name, entry.value
            ))
        })?),
        ServoRegisterWidth::U16 => entry.value,
    };

    if observed != expected {
        return Err(HalError::Communication(format!(
            "EEPROM validation failed for servo {} entry {} at address {}: expected {}, observed {}",
            servo_id, entry.name, entry.address, expected, observed
        )));
    }

    Ok(observed)
}

pub fn validate_servo_eeprom_entry(
    bus: &mut RealStsBus,
    servo_id: u8,
    entry: &ServoEepromEntry,
) -> HalResult<()> {
    let _ = validate_servo_eeprom_entry_value(bus, servo_id, entry)?;
    Ok(())
}

pub fn set_verified_torque_limit_on_current_position_for_ids(
    bus: &mut RealStsBus,
    servo_ids: &[u8],
    torque_limit: u16,
) -> HalResult<()> {
    thread::sleep(Duration::from_millis(VERIFIED_TORQUE_LIMIT_PHASE_SETTLE_MS));

    for &servo_id in servo_ids {
        sync_current_pose(bus, &[servo_id]).map_err(|err| {
            HalError::Communication(format!(
                "failed to sync current position for servo {servo_id}: {err}"
            ))
        })?;
        set_verified_servo_torque_limit(bus, servo_id, torque_limit).map_err(|err| {
            HalError::Communication(format!(
                "failed to set verified torque limit on servo {servo_id}: {err}"
            ))
        })?;
    }

    Ok(())
}

fn set_verified_servo_torque_limit(
    bus: &mut RealStsBus,
    servo_id: u8,
    expected_torque_limit: u16,
) -> HalResult<()> {
    let mut last_error = None;

    for attempt in 1..=VERIFIED_TORQUE_LIMIT_VERIFY_ATTEMPTS {
        match bus.set_servo_torque_limit(servo_id, expected_torque_limit) {
            Ok(()) => {
                thread::sleep(Duration::from_millis(VERIFIED_TORQUE_LIMIT_APPLY_SLEEP_MS));
                for read_attempt in 1..=VERIFIED_TORQUE_LIMIT_READBACK_ATTEMPTS {
                    match bus.read_servo_torque_limit(servo_id) {
                        Ok(observed) if observed == expected_torque_limit => return Ok(()),
                        Ok(observed) => {
                            last_error = Some(format!(
                                "expected torque limit {}, observed {} on attempt {}.{}",
                                expected_torque_limit, observed, attempt, read_attempt
                            ));
                            break;
                        }
                        Err(err) => {
                            let feedback_hint = match bus.read_feedback(servo_id) {
                                Ok(_) => "feedback ok".to_owned(),
                                Err(feedback_err) => {
                                    format!("feedback probe failed: {feedback_err}")
                                }
                            };
                            last_error = Some(format!(
                                "readback failed on attempt {}.{}: {err}; {}",
                                attempt, read_attempt, feedback_hint
                            ));
                            if read_attempt < VERIFIED_TORQUE_LIMIT_READBACK_ATTEMPTS {
                                thread::sleep(Duration::from_millis(
                                    VERIFIED_TORQUE_LIMIT_VERIFY_SLEEP_MS,
                                ));
                            }
                        }
                    }
                }
            }
            Err(err) => {
                last_error = Some(format!("write failed on attempt {}: {err}", attempt));
            }
        }

        if attempt < VERIFIED_TORQUE_LIMIT_VERIFY_ATTEMPTS {
            thread::sleep(Duration::from_millis(VERIFIED_TORQUE_LIMIT_VERIFY_SLEEP_MS));
        }
    }

    Err(HalError::Communication(last_error.unwrap_or_else(|| {
        "torque limit write did not complete".to_owned()
    })))
}

impl ServoBus for RealStsBus {
    fn servo_ids(&self) -> &[u8] {
        &self.servo_ids
    }

    // Attention: enabling torque on all servos at once can cause a sudden motion if any of them are not already at their current position,
    // so this method should be used with caution. For example, it's best to call this right after setting the desired position for all servos
    // with sync_write_positions, and avoid calling it while the robot is in the air.
    fn enable_torque(&mut self, enabled: bool) -> HalResult<()> {
        for &servo_id in &self.servo_ids.clone() {
            self.write_runtime_byte(servo_id, TORQUE_ENABLE.address, u8::from(enabled))?;
        }

        Ok(())
    }

    fn sync_write_positions(&mut self, commands: &[JointCommand]) -> HalResult<()> {
        let _ = runtime_writable_register(GOAL_POSITION.address, GOAL_POSITION.width_bytes)?;
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
        params.push(GOAL_POSITION.address);
        params.push(GOAL_POSITION.width_bytes);

        for command in commands {
            let [low, high] = command.position_ticks.to_le_bytes();
            params.push(command.servo_id);
            params.push(low);
            params.push(high);
        }

        let packet = pack_instruction(0xFE, Instruction::SyncWrite, &params);
        // Broadcast SYNC_WRITE does not produce per-servo status packets, even when
        // Status Return Level is 1. Post-write validation therefore still happens
        // through later feedback reads rather than an immediate write acknowledgement.
        trace!(
            target: "arachno_feetech_sts::bus",
            address = GOAL_POSITION.address,
            command_count = commands.len(),
            commands = %format_joint_commands(commands),
            packet = %format_bytes_hex(&packet),
            "tx sync-write"
        );
        self.port.write_all(&packet).map_err(|err| {
            trace!(
                target: "arachno_feetech_sts::bus",
                error = %err,
                "tx-error sync-write"
            );
            HalError::Communication(format!("sync write failed: {err}"))
        })?;
        self.port.flush().map_err(|err| {
            trace!(
                target: "arachno_feetech_sts::bus",
                error = %err,
                "tx-flush-error sync-write"
            );
            HalError::Communication(format!("sync write flush failed: {err}"))
        })?;

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

fn format_bytes_hex(data: &[u8]) -> String {
    data.iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_joint_commands(commands: &[JointCommand]) -> String {
    commands
        .iter()
        .map(|command| format!("{}:{}", command.servo_id, command.position_ticks))
        .collect::<Vec<_>>()
        .join(",")
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

fn runtime_writable_register(address: u8, width_bytes: u8) -> HalResult<&'static ServoRegister> {
    let Some(register) = lookup_register(address, width_bytes) else {
        return Err(HalError::Unsupported(format!(
            "runtime write to unknown register {} ({} byte{}) is not allowed",
            address,
            width_bytes,
            if width_bytes == 1 { "" } else { "s" }
        )));
    };

    if register.area != Eeprom {
        return Ok(register);
    }

    let detail = format!(
        "{} [{} {:?} {:?}]",
        register.name, register.address, register.area, register.access
    );

    Err(HalError::Unsupported(format!(
        "runtime write is not allowed for {detail}"
    )))
}

fn persistent_writable_register(address: u8, width_bytes: u8) -> HalResult<&'static ServoRegister> {
    let Some(register) = lookup_register(address, width_bytes) else {
        return Err(HalError::Unsupported(format!(
            "persistent write to unknown register {} ({} byte{}) is not allowed",
            address,
            width_bytes,
            if width_bytes == 1 { "" } else { "s" }
        )));
    };

    if register.area != Eeprom || register.access != ReadWrite {
        return Err(HalError::Unsupported(format!(
            "persistent write is not allowed for {} [{} {:?} {:?}]",
            register.name, register.address, register.area, register.access
        )));
    }

    Ok(register)
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
    use super::{
        GOAL_POSITION, LOCK_MARK, MAX_TORQUE_LIMIT, PRESENT_TELEMETRY, STATUS_RETURN_LEVEL,
        TORQUE_ENABLE, TORQUE_LIMIT, calculate_checksum, decode_faults, load_raw_to_pct,
        parse_status_packet, persistent_writable_register, raw_current_to_ma,
        runtime_writable_register,
    };

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

    #[test]
    fn parses_empty_write_confirmation_packet() {
        let packet = [0xFF, 0xFF, 0x01, 0x02, 0x00, 0xFC];
        let status = parse_status_packet(1, &packet).expect("write confirmation should parse");

        assert_eq!(status.status, 0);
        assert!(status.params.is_empty());
    }

    #[test]
    fn runtime_write_rejects_only_eeprom_registers() {
        assert!(
            runtime_writable_register(TORQUE_ENABLE.address, TORQUE_ENABLE.width_bytes).is_ok()
        );
        assert!(runtime_writable_register(TORQUE_LIMIT.address, TORQUE_LIMIT.width_bytes).is_ok());
        assert!(
            runtime_writable_register(GOAL_POSITION.address, GOAL_POSITION.width_bytes).is_ok()
        );
        assert!(
            runtime_writable_register(PRESENT_TELEMETRY.address, PRESENT_TELEMETRY.width_bytes)
                .is_ok()
        );
        assert!(runtime_writable_register(LOCK_MARK.address, LOCK_MARK.width_bytes).is_ok());
        assert!(
            runtime_writable_register(MAX_TORQUE_LIMIT.address, MAX_TORQUE_LIMIT.width_bytes)
                .is_err()
        );
        assert!(
            runtime_writable_register(STATUS_RETURN_LEVEL.address, STATUS_RETURN_LEVEL.width_bytes)
                .is_err()
        );
    }

    #[test]
    fn persistent_write_rejects_ram_registers() {
        assert!(
            persistent_writable_register(
                STATUS_RETURN_LEVEL.address,
                STATUS_RETURN_LEVEL.width_bytes
            )
            .is_ok()
        );
        assert!(
            persistent_writable_register(MAX_TORQUE_LIMIT.address, MAX_TORQUE_LIMIT.width_bytes)
                .is_ok()
        );
        assert!(
            persistent_writable_register(TORQUE_ENABLE.address, TORQUE_ENABLE.width_bytes).is_err()
        );
        assert!(persistent_writable_register(LOCK_MARK.address, LOCK_MARK.width_bytes).is_err());
        assert!(
            persistent_writable_register(TORQUE_LIMIT.address, TORQUE_LIMIT.width_bytes).is_err()
        );
    }
}
