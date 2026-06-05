#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterArea {
    Eeprom,
    Sram,
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAccess {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServoRegister {
    pub name: &'static str,
    pub address: u8,
    pub width_bytes: u8,
    pub area: RegisterArea,
    pub access: RegisterAccess,
}

impl ServoRegister {
    pub const fn new(
        name: &'static str,
        address: u8,
        width_bytes: u8,
        area: RegisterArea,
        access: RegisterAccess,
    ) -> Self {
        Self {
            name,
            address,
            width_bytes,
            area,
            access,
        }
    }
}

pub const STATUS_RETURN_LEVEL: ServoRegister = ServoRegister::new(
    "Status Return Level",
    8,
    1,
    RegisterArea::Eeprom,
    RegisterAccess::ReadWrite,
);

pub const MAX_TORQUE_LIMIT: ServoRegister = ServoRegister::new(
    "Max Torque Limit",
    16,
    2,
    RegisterArea::Eeprom,
    RegisterAccess::ReadWrite,
);

pub const TORQUE_ENABLE: ServoRegister = ServoRegister::new(
    "Torque Enable",
    40,
    1,
    RegisterArea::Sram,
    RegisterAccess::ReadWrite,
);

pub const GOAL_POSITION: ServoRegister = ServoRegister::new(
    "Goal Position",
    42,
    2,
    RegisterArea::Sram,
    RegisterAccess::ReadWrite,
);

pub const TORQUE_LIMIT: ServoRegister = ServoRegister::new(
    "Torque Limit",
    48,
    2,
    RegisterArea::Sram,
    RegisterAccess::ReadWrite,
);

pub const LOCK_MARK: ServoRegister = ServoRegister::new(
    "Lock Mark",
    55,
    1,
    RegisterArea::Sram,
    RegisterAccess::ReadWrite,
);

pub const PRESENT_TELEMETRY: ServoRegister = ServoRegister::new(
    "Present Telemetry Block",
    56,
    15,
    RegisterArea::Sram,
    RegisterAccess::ReadOnly,
);

pub const KNOWN_REGISTERS: &[ServoRegister] = &[
    STATUS_RETURN_LEVEL,
    MAX_TORQUE_LIMIT,
    TORQUE_ENABLE,
    GOAL_POSITION,
    TORQUE_LIMIT,
    LOCK_MARK,
    PRESENT_TELEMETRY,
];

pub fn lookup_register(address: u8, width_bytes: u8) -> Option<&'static ServoRegister> {
    KNOWN_REGISTERS
        .iter()
        .find(|register| register.address == address && register.width_bytes == width_bytes)
}
