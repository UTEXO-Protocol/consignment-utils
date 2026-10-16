use serde::{Deserialize, Serialize};

/// Top-level summary of a parsed consignment file.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConsignmentInfo {
    Transfer(TransferInfo),
    Contract(ContractInfo),
}

impl ConsignmentInfo {
    pub fn kind(&self) -> ConsignmentKind {
        match self {
            Self::Transfer(_) => ConsignmentKind::Transfer,
            Self::Contract(_) => ConsignmentKind::Contract,
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConsignmentKind {
    Transfer,
    Contract,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransferInfo {
    /// Container format version (`1` for the V1 containers of RGB 0.11.2).
    pub version: u8,
    /// Genesis (issuance) info.
    pub genesis: GenesisInfo,
    /// Schema identifier (baid64), the one genesis commits to.
    pub schema_id: String,
    /// History terminal seal bundles.
    pub terminals: Vec<TerminalInfo>,
    /// Witness transactions referenced by bundles.
    pub witnesses: Vec<WitnessInfo>,
    /// Total number of transition bundles in the consignment.
    pub bundle_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractInfo {
    pub version: u8,
    pub genesis: GenesisInfo,
    pub schema_id: String,
    pub terminals: Vec<TerminalInfo>,
    pub witnesses: Vec<WitnessInfo>,
    pub bundle_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenesisInfo {
    /// Contract id (baid64), i.e. the asset id.
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
    /// Number of global state types declared at genesis (not values).
    pub global_state_count: u32,
    /// Number of asset-tag declarations (one per fungible state type).
    pub assignment_count: u32,
    /// Fungible allocations issued at genesis, grouped by assignment type.
    /// For an NIA asset this is the issuance breakdown: `total` is the
    /// total supply.
    pub fungible_allocations: Vec<FungibleAllocation>,
    /// Asset name. Decoded from `GS_NOMINAL`'s `AssetSpec` (NIA/IFA/PFA/BFA)
    /// or from `GS_NAME` directly (CFA). `None` when the schema doesn't carry
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
    /// Global state declared at genesis, one entry per value (e.g. BFA
    /// `bridgeLocation` = 3008).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub globals: Vec<GlobalEntry>,
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
    /// Concrete `txid:vout`. `txid` is `None` when the seal points at the
    /// witness tx of its bundle: the `WitnessInfo` holding the transition,
    /// or for a terminal one with the same `bundle_id`. Nothing is validated,
    /// so that can match more than one `WitnessInfo`.
    Revealed {
        #[serde(skip_serializing_if = "Option::is_none")]
        txid: Option<String>,
        vout: u32,
    },
    /// Recipient seal kept hidden - only the SHA-256 hash is exposed.
    Confidential { secret_seal: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransitionInfo {
    /// Operation id (hex).
    pub op_id: String,
    /// Schema-defined transition-type id.
    pub transition_type: u16,
    /// Number of inputs consumed by this transition.
    pub input_count: u32,
    /// Outputs grouped by assignment type. Empty if the transition only
    /// moves declarative or structured (non-fungible) state.
    pub fungible_allocations: Vec<FungibleAllocation>,
    /// Schema-defined metadata carried by the transition (e.g. BFA mint
    /// `afterBlock` = 1003), raw strict-encoded bytes as lowercase hex.
    /// Empty when there is none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metadata: Vec<MetaEntry>,
    /// Global state committed by the transition, one entry per value (e.g.
    /// BFA burn `burnedAsset` = 2015, `burnReason` = 3009). Empty when there
    /// is none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub globals: Vec<GlobalEntry>,
}

/// One metadata field of a transition.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetaEntry {
    /// Schema-defined metadata type id.
    pub meta_type: u16,
    /// Raw strict-encoded value, lowercase hex without prefix.
    pub value_hex: String,
}

/// One global state value of an operation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlobalEntry {
    /// Schema-defined global state type id.
    pub global_type: u16,
    /// Raw strict-encoded value, lowercase hex without prefix.
    pub value_hex: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerminalInfo {
    /// Bundle id (hex), as in `WitnessInfo.bundle_id`; unvalidated input can repeat
    /// a bundle, so more than one `WitnessInfo` may match.
    pub bundle_id: String,
    /// Terminal seals of this bundle: concealed for blinded receives,
    /// revealed for witness-vout ones.
    pub seals: Vec<SealInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WitnessInfo {
    /// Bitcoin txid the bundle commits to.
    pub txid: String,
    /// Id (hex) of the bundle this tx witnesses.
    pub bundle_id: String,
    /// Number of state transitions in the bundle.
    pub transition_count: u32,
    /// Number of inputs referenced from the witness tx.
    pub input_count: u32,
    /// State transitions in this bundle, with their fungible outputs.
    pub transitions: Vec<TransitionInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_json_uses_internally_tagged_kind() {
        let info = ConsignmentInfo::Transfer(TransferInfo {
            version: 1,
            genesis: GenesisInfo {
                contract_id: "c".into(),
                schema_id: "s".into(),
                chain_net: "bc".into(),
                timestamp: 1,
                issuer: String::new(),
                ffv: "RGB/1.0".into(),
                global_state_count: 0,
                assignment_count: 0,
                fungible_allocations: vec![],
                name: Some("Name".into()),
                ticker: Some("TKN".into()),
                precision: Some(8),
                details: None,
                globals: vec![],
            },
            schema_id: "s".into(),
            terminals: vec![],
            witnesses: vec![],
            bundle_count: 0,
        });
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains(r#""kind":"transfer""#), "{json}");
        assert!(json.contains(r#""ticker":"TKN""#), "{json}");
        assert!(!json.contains("details"), "{json}");
        assert!(!json.contains("globals"), "{json}");
    }

    fn bare_transition() -> TransitionInfo {
        TransitionInfo {
            op_id: "op".into(),
            transition_type: 8010,
            input_count: 1,
            fungible_allocations: vec![],
            metadata: vec![],
            globals: vec![],
        }
    }

    #[test]
    fn transition_metadata_round_trips_and_is_omitted_when_empty() {
        let mut t = bare_transition();
        let json = serde_json::to_string(&t).unwrap();
        assert!(!json.contains("metadata"), "{json}");
        let back: TransitionInfo = serde_json::from_str(&json).unwrap();
        assert!(back.metadata.is_empty());

        t.metadata = vec![MetaEntry {
            meta_type: 1003,
            value_hex: "2a00000000000000".into(),
        }];
        let json = serde_json::to_string(&t).unwrap();
        assert!(
            json.contains(r#""meta_type":1003,"value_hex":"2a00000000000000""#),
            "{json}"
        );
        let back: TransitionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.metadata, t.metadata);
    }

    #[test]
    fn transition_globals_round_trip_and_are_omitted_when_empty() {
        let mut t = bare_transition();
        let json = serde_json::to_string(&t).unwrap();
        assert!(!json.contains("globals"), "{json}");
        let back: TransitionInfo = serde_json::from_str(&json).unwrap();
        assert!(back.globals.is_empty());

        t.globals = vec![
            GlobalEntry {
                global_type: 2015,
                value_hex: "1027000000000000".into(),
            },
            GlobalEntry {
                global_type: 3009,
                value_hex: "00".repeat(12) + &"ab".repeat(20),
            },
        ];
        let json = serde_json::to_string(&t).unwrap();
        assert!(
            json.contains(r#""global_type":2015,"value_hex":"1027000000000000""#),
            "{json}"
        );
        let back: TransitionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.globals, t.globals);
    }
}
