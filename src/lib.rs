//! RGB consignment kit.
//!
//! Accepts the binary container format produced by `rgb-std` / `rgb-lib`
//! (`.rgb` files: Transfer, Contract, or Kit) and returns a flat
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
    KitInfo, SealInfo, TerminalInfo, TransferInfo, TransitionInfo, WitnessInfo,
};
pub use parse::{ParseError, parse};
