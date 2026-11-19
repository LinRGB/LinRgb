//! Closed registry: adding a target requires declaring its evidence, access and worker policy.
//! Active verification lives in Target::connect; runtime facts only come from Session.
use super::{Interface, Target, aura::Zone};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    Direct,
    Memory,
    Aura,
    Wdl,
}
impl Lane {
    pub const ALL: [Self; 4] = [Self::Direct, Self::Memory, Self::Aura, Self::Wdl];
}
pub struct Driver {
    pub id: &'static str,
    pub evidence: &'static str,
    pub interface: Interface,
    pub lane: Lane,
}
impl Target {
    pub fn driver(&self) -> Driver {
        use Interface::*;
        use Lane::*;
        let (id, evidence, interface, lane) = match self {
            Self::Kernel(..) => (
                "linux-multicolor",
                "kernel-interface",
                SysfsAttributes,
                Direct,
            ),
            Self::LampArray(_) => ("hid-lamparray", "known-report-layout", HidrawFeatures, Wdl),
            Self::Aura(_, zone) => (
                match zone {
                    Zone::Fixed => "aura-fixed",
                    Zone::Header1 => "aura-argb",
                },
                "board-and-aura-interface",
                HidrawWrite,
                Aura,
            ),
            Self::EneGpu(_) => ("ene-gpu", "pci-candidate", Smbus, Direct),
            Self::Ram(_) => ("ene-dram", "slot-candidate", Smbus, Memory),
            Self::Corsair(..) => ("corsair-dram", "slot-candidate", Smbus, Memory),
        };
        Driver {
            id,
            evidence,
            interface,
            lane,
        }
    }
}
