use amplify::Wrapper;
use amplify::confinement::U32 as FILE_MAX_LEN;
use rgbstd::containers::{Consignment, WitnessBundle};
use rgbstd::stl::{AssetSpec, Details, Name};
use rgbstd::{
    Assign, Assignments, BuilderSeal, ExposedSeal, Genesis, GlobalState, GlobalStateType,
    KnownTransition, Operation, Precision, RevealedValue, TypedAssigns,
};
use strict_types::encoding::{StreamReader, StrictDecode, StrictDeserialize};

use crate::info::{
    ConsignmentInfo, ContractInfo, FungibleAllocation, FungibleEntry, GenesisInfo, GlobalEntry,
    MetaEntry, SealInfo, TerminalInfo, TransferInfo, TransitionInfo, WitnessInfo,
};

// Well-known global-state-type IDs from `rgb-schemas`.
// Inlined to avoid pulling the full schemas crate as a dep.
const GS_NOMINAL: u16 = 2000; // RGBContract.AssetSpec (NIA / IFA / PFA / UDA / BFA)
const GS_NAME: u16 = 3001; // RGBContract.Name (CFA)
const GS_DETAILS: u16 = 3004; // RGBContract.Details
const GS_PRECISION: u16 = 3005; // RGBContract.Precision

const RGB_PREFIX: &[u8] = b"RGB\x00";
const MAGIC_LEN: usize = 3;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("not a valid RGB consignment file: {0}")]
    Decode(String),
}

/// Parse a binary consignment file into a flat summary.
///
/// Accepts the V1 containers written by RGB 0.11.2: `TFR` / `CON` and the
/// validated copies `VTF` / `VCO` that rgb-lib keeps on disk.
pub fn parse(bytes: &[u8]) -> Result<ConsignmentInfo, ParseError> {
    let (magic, body) = bytes
        .strip_prefix(RGB_PREFIX)
        .and_then(|rest| rest.split_at_checked(MAGIC_LEN))
        .ok_or_else(|| ParseError::Decode("missing RGB file header".into()))?;
    match magic {
        b"TFR" | b"VTF" => Ok(ConsignmentInfo::Transfer(transfer_info(&load(body)?))),
        b"CON" | b"VCO" => Ok(ConsignmentInfo::Contract(contract_info(&load(body)?))),
        _ => Err(ParseError::Decode(format!(
            "unsupported container type {:?}",
            String::from_utf8_lossy(magic)
        ))),
    }
}

// The validated copies append their validation status as JSON, which is not read.
fn load<const TRANSFER: bool>(body: &[u8]) -> Result<Consignment<TRANSFER>, ParseError> {
    let reader = StreamReader::new::<FILE_MAX_LEN>(body);
    let consignment = Consignment::<TRANSFER>::strict_read(reader)
        .map_err(|e| ParseError::Decode(e.to_string()))?;
    if consignment.transfer != TRANSFER {
        return Err(ParseError::Decode(
            "container type does not match the consignment transfer flag".into(),
        ));
    }
    Ok(consignment)
}

fn transfer_info(c: &Consignment<true>) -> TransferInfo {
    TransferInfo {
        version: u8::from(c.version),
        genesis: genesis_info(&c.genesis),
        schema_id: c.schema_id().to_string(),
        terminals: terminal_infos(c),
        witnesses: c.bundles.iter().map(witness_info).collect(),
        bundle_count: c.bundles.len() as u32,
    }
}

fn contract_info(c: &Consignment<false>) -> ContractInfo {
    ContractInfo {
        version: u8::from(c.version),
        genesis: genesis_info(&c.genesis),
        schema_id: c.schema_id().to_string(),
        terminals: terminal_infos(c),
        witnesses: c.bundles.iter().map(witness_info).collect(),
        bundle_count: c.bundles.len() as u32,
    }
}

fn terminal_infos<const TRANSFER: bool>(c: &Consignment<TRANSFER>) -> Vec<TerminalInfo> {
    c.terminals
        .iter()
        .map(|(bundle_id, seals)| TerminalInfo {
            bundle_id: bundle_id.to_string(),
            seals: seals.iter().map(seal_info).collect(),
        })
        .collect()
}

fn witness_info(wb: &WitnessBundle) -> WitnessInfo {
    let bundle = &wb.bundle;
    WitnessInfo {
        txid: wb.witness_id().to_string(),
        bundle_id: bundle.bundle_id().to_string(),
        transition_count: bundle.known_transitions.len() as u32,
        input_count: bundle.input_map.len() as u32,
        transitions: bundle
            .known_transitions
            .iter()
            .map(transition_info)
            .collect(),
    }
}

fn genesis_info(g: &Genesis) -> GenesisInfo {
    let asset = AssetMeta::from_globals(&g.globals);
    GenesisInfo {
        contract_id: g.contract_id().to_string(),
        schema_id: g.schema_id.to_string(),
        chain_net: g.chain_net.prefix().to_string(),
        timestamp: g.timestamp,
        issuer: g.issuer.to_string(),
        ffv: g.ffv.to_string(),
        global_state_count: g.globals.len() as u32,
        assignment_count: g.assignments.len() as u32,
        fungible_allocations: fungible_allocations(&g.assignments),
        name: asset.name,
        ticker: asset.ticker,
        precision: asset.precision,
        details: asset.details,
        globals: global_entries(&g.globals),
    }
}

/// Asset metadata decoded from genesis global state. `None` means the schema
/// doesn't carry the field or its value did not decode.
struct AssetMeta {
    name: Option<String>,
    ticker: Option<String>,
    precision: Option<u8>,
    details: Option<String>,
}

impl AssetMeta {
    // AssetSpec carries all four fields; CFA keeps them as separate globals.
    fn from_globals(globals: &GlobalState) -> Self {
        if let Some(spec) = decode_global::<AssetSpec>(globals, GS_NOMINAL) {
            return Self {
                name: Some(spec.name.to_string()),
                ticker: Some(spec.ticker.to_string()),
                precision: Some(spec.precision.decimals()),
                details: spec.details.map(|d| d.to_string()),
            };
        }
        Self {
            name: decode_global::<Name>(globals, GS_NAME).map(|n| n.to_string()),
            ticker: None,
            precision: decode_global::<Precision>(globals, GS_PRECISION).map(Precision::decimals),
            details: decode_global::<Details>(globals, GS_DETAILS).map(|d| d.to_string()),
        }
    }
}

// V1 consignments embed no type system, so values decode against the known Rust types.
fn decode_global<T: StrictDeserialize>(globals: &GlobalState, global_type: u16) -> Option<T> {
    let value = globals.get(&GlobalStateType::with(global_type))?.first()?;
    T::from_strict_serialized(value.to_inner()).ok()
}

fn global_entries(globals: &GlobalState) -> Vec<GlobalEntry> {
    globals
        .iter()
        .flat_map(|(ty, values)| {
            values.iter().map(move |value| GlobalEntry {
                global_type: ty.to_inner(),
                value_hex: to_hex(value.as_slice()),
            })
        })
        .collect()
}

fn transition_info(kt: &KnownTransition) -> TransitionInfo {
    let t = &kt.transition;
    TransitionInfo {
        op_id: kt.opid.to_string(),
        transition_type: t.transition_type.to_inner(),
        input_count: t.inputs.len() as u32,
        fungible_allocations: fungible_allocations(&t.assignments),
        metadata: t
            .metadata
            .iter()
            .map(|(ty, value)| MetaEntry {
                meta_type: ty.to_inner(),
                value_hex: to_hex(value.as_ref()),
            })
            .collect(),
        globals: global_entries(&t.globals),
    }
}

fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 0x0f) as usize] as char);
    }
    out
}

fn fungible_allocations<Seal: ExposedSeal>(asgs: &Assignments<Seal>) -> Vec<FungibleAllocation> {
    asgs.iter()
        .filter_map(|(ty, ta)| {
            let TypedAssigns::Fungible(vec) = ta else {
                return None;
            };
            let entries: Vec<FungibleEntry> = vec.iter().map(fungible_entry).collect();
            let total = entries.iter().map(|e| e.amount).sum();
            Some(FungibleAllocation {
                assignment_type: u16::from_le_bytes(ty.to_le_bytes()),
                entries,
                total,
            })
        })
        .collect()
}

fn fungible_entry<Seal: ExposedSeal>(a: &Assign<RevealedValue, Seal>) -> FungibleEntry {
    FungibleEntry {
        amount: (*a.state.as_inner()).into(),
        seal: seal_info(&a.seal),
    }
}

fn seal_info<Seal: ExposedSeal>(seal: &BuilderSeal<Seal>) -> SealInfo {
    match seal {
        BuilderSeal::Revealed(seal) => SealInfo::Revealed {
            txid: seal.txid().map(|t| t.to_string()),
            vout: seal.vout().to_u32(),
        },
        BuilderSeal::Concealed(secret) => SealInfo::Confidential {
            secret_seal: secret.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bytes_without_rgb_header() {
        let err = parse(b"nope").unwrap_err();
        assert!(err.to_string().contains("missing RGB file header"), "{err}");
    }

    #[test]
    fn rejects_schema_definition_files() {
        let err = parse(b"RGB\x00SD2\x00").unwrap_err();
        assert!(
            err.to_string().contains("unsupported container type"),
            "{err}"
        );
    }

    #[test]
    fn rejects_truncated_body() {
        assert!(parse(b"RGB\x00TFR\x01").is_err());
    }

    #[test]
    fn hex_is_lowercase_without_prefix() {
        assert_eq!(to_hex(&[0x00, 0xab, 0x10]), "00ab10");
    }
}
