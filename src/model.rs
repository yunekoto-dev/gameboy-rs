#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwareModel {
    Dmg,
    Cgb,
}

impl HardwareModel {
    pub const fn is_cgb(self) -> bool {
        matches!(self, Self::Cgb)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Dmg => "DMG",
            Self::Cgb => "CGB",
        }
    }
}
