use serde::{Deserialize, Serialize};

/// Top-level summary of a parsed consignment file.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConsignmentInfo {
    Transfer(TransferInfo),
    Contract(ContractInfo),
    Kit(KitInfo),
}

impl ConsignmentInfo {
    pub fn kind(&self) -> ConsignmentKind {
        match self {
            Self::Transfer(_) => ConsignmentKind::Transfer,
            Self::Contract(_) => ConsignmentKind::Contract,
            Self::Kit(_) => ConsignmentKind::Kit,
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConsignmentKind {
    Transfer,
    Contract,
    Kit,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransferInfo {
    /// Container format version (`v2`/`v3`/...).
    pub version: u8,
    /// Genesis (issuance) info.
    pub genesis: GenesisInfo,
    /// Schema identifier (baid64).
    pub schema_id: String,
    /// History terminal seal bundles.
    pub terminals: Vec<TerminalInfo>,
    /// Witness transactions referenced by bundles.
    pub witnesses: Vec<WitnessInfo>,
    /// Total number of transition bundles in the consignment.
    pub bundle_count: u32,
    /// Number of validation script libraries embedded.
    pub script_count: u32,
    /// Number of strict types embedded.
    pub types_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractInfo {
    pub version: u8,
    pub genesis: GenesisInfo,
    pub schema_id: String,
    pub terminals: Vec<TerminalInfo>,
    pub witnesses: Vec<WitnessInfo>,
    pub bundle_count: u32,
    pub script_count: u32,
    pub types_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KitInfo {
    pub version: u8,
    pub schema_count: u32,
    pub script_count: u32,
    pub types_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenesisInfo {
    /// Contract id (baid64) — i.e. the asset id.
    pub contract_id: String,
    /// Schema id (baid64) the contract was issued under.
    pub schema_id: String,
    /// Bitcoin chain / network the contract is anchored to (e.g. `bc:regtest`).
    pub chain_net: String,
    /// Genesis timestamp (Unix seconds).
    pub timestamp: i64,
    /// Issuer identity string (may be empty).
    pub issuer: String,
    /// RGB fast-forward version code (display form, e.g. `RGB/1.0`).
    pub ffv: String,
    /// Number of global state entries declared at genesis.
    pub global_state_count: u32,
    /// Number of asset-tag declarations (one per fungible state type).
    pub assignment_count: u32,
    /// Fungible allocations issued at genesis, grouped by assignment type.
    /// For an NIA asset this is the issuance breakdown — `total` is the
    /// total supply.
    pub fungible_allocations: Vec<FungibleAllocation>,
    /// Asset name. Decoded from `GS_NOMINAL`'s `AssetSpec` (NIA/IFA/PFA) or
    /// from `GS_NAME` directly (CFA). `None` when the schema doesn't carry
    /// either, or decoding failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Asset ticker / symbol. Decoded from `GS_NOMINAL`'s `AssetSpec`. `None`
    /// for schemas without a ticker (e.g. CFA) or when decoding failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticker: Option<String>,
    /// Asset precision (number of decimal places in the asset's smallest
    /// unit). Decoded from `AssetSpec.precision` or `GS_PRECISION`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precision: Option<u8>,
    /// Free-form asset description, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// All fungible outputs of a single assignment type (e.g. `assetOwner`),
/// together with the sum.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FungibleAllocation {
    /// Schema-defined assignment-type id (decimal `u16`).
    pub assignment_type: u16,
    /// One entry per output / allocation.
    pub entries: Vec<FungibleEntry>,
    /// Sum of all `entries[].amount`.
    pub total: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FungibleEntry {
    /// Plain amount in the asset's smallest unit.
    pub amount: u64,
    /// The seal that owns this allocation.
    pub seal: SealInfo,
}

/// Where an allocation lives. RGB 0.11 hides only the seal (never the
/// amount), so this distinguishes "we know the txout" from "only the
/// recipient knows the txout".
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SealInfo {
    /// Concrete `txid:vout`. For transition seals, `txid` is `None` when
    /// the seal points at the witness tx of its containing bundle —
    /// resolve by combining with `WitnessInfo.txid`.
    Revealed {
        #[serde(skip_serializing_if = "Option::is_none")]
        txid: Option<String>,
        vout: u32,
    },
    /// Recipient seal kept hidden — only the SHA-256 hash is exposed.
    Confidential { secret_seal: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransitionInfo {
    /// Operation id (baid64).
    pub op_id: String,
    /// Schema-defined transition-type id.
    pub transition_type: u16,
    /// Number of inputs consumed by this transition.
    pub input_count: u32,
    /// Outputs grouped by assignment type. Empty if the transition only
    /// moves declarative or structured (non-fungible) state.
    pub fungible_allocations: Vec<FungibleAllocation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerminalInfo {
    /// Bundle id (baid64).
    pub bundle_id: String,
    /// Number of secret seals listed at this terminal.
    pub secret_seal_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WitnessInfo {
    /// Bitcoin txid the bundle commits to.
    pub txid: String,
    /// Number of state transitions in the bundle.
    pub transition_count: u32,
    /// Number of inputs referenced from the witness tx.
    pub input_count: u32,
    /// State transitions in this bundle, with their fungible outputs.
    pub transitions: Vec<TransitionInfo>,
}
