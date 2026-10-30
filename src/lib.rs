//! RGB consignment kit.
//!
//! Accepts the binary V1 containers produced by `rgb-ops` / `rgb-lib` on RGB
//! 0.11.2 (Transfer or Contract, plain or validated) and returns a flat
//! [`ConsignmentInfo`] summary suitable for serialization across language
//! boundaries.
//!
//! ```no_run
//! let bytes = std::fs::read("transfer.rgb").unwrap();
//! let info = rgb_consignment::parse(&bytes).unwrap();
//! println!("{}", serde_json::to_string_pretty(&info).unwrap());
//! ```
//!
//! A C ABI (`rgb_consignment_parse`) is exported for language bindings.
//! The Go wrapper lives in `rgbconsignment`.

mod ffi;
mod info;
mod parse;

pub use info::{
    ConsignmentInfo, ConsignmentKind, ContractInfo, FungibleAllocation, FungibleEntry, GenesisInfo,
    GlobalEntry, MetaEntry, SealInfo, TransferInfo, TransitionInfo, WitnessInfo,
};
pub use parse::{ParseError, parse};
