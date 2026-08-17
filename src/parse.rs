use amplify::Wrapper;
use rgbstd::containers::{Consignment, ConsignmentExt, Kit, UniversalFile};
use rgbstd::schema::GlobalStateType;
use rgbstd::{Assign, Assignments, ExposedSeal, KnownTransition, Transition, TypedAssigns};
use strict_types::value::{EnumTag, StrictVal};
use strict_types::{FieldName, SemId, Ty, TypeSystem, VariantName};

use crate::info::{
    ConsignmentInfo, ContractInfo, FungibleAllocation, FungibleEntry, GenesisInfo, KitInfo,
    SealInfo, TerminalInfo, TransferInfo, TransitionInfo, WitnessInfo,
};

// Well-known global-state-type IDs from `rgb-schemas`.
// Inlined to avoid pulling the full schemas crate as a dep.
const GS_NOMINAL: u16 = 2000; // RGBContract.AssetSpec (NIA / IFA / PFA / UDA)
const GS_NAME: u16 = 3001; // RGBContract.Name (CFA)
const GS_DETAILS: u16 = 3004; // RGBContract.Details
const GS_PRECISION: u16 = 3005; // RGBContract.Precision

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("not a valid RGB consignment file: {0}")]
    Decode(String),
}

impl From<rgbstd::containers::LoadError> for ParseError {
    fn from(e: rgbstd::containers::LoadError) -> Self {
        Self::Decode(e.to_string())
    }
}

/// Parse a binary consignment file into a flat summary.
pub fn parse(bytes: &[u8]) -> Result<ConsignmentInfo, ParseError> {
    let file = UniversalFile::load(bytes)?;
    Ok(match file {
        UniversalFile::Transfer(t) => ConsignmentInfo::Transfer(transfer_info(&t)),
        UniversalFile::Contract(c) => ConsignmentInfo::Contract(contract_info(&c)),
        UniversalFile::Kit(k) => ConsignmentInfo::Kit(kit_info(&k)),
    })
}

fn transfer_info(c: &Consignment<true>) -> TransferInfo {
    let (genesis, schema_id, terminals, witnesses, bundle_count, script_count, types_count) =
        consignment_common(c);
    TransferInfo {
        version: c.version as u8,
        genesis,
        schema_id,
        terminals,
        witnesses,
        bundle_count,
        script_count,
        types_count,
    }
}

fn contract_info(c: &Consignment<false>) -> ContractInfo {
    let (genesis, schema_id, terminals, witnesses, bundle_count, script_count, types_count) =
        consignment_common(c);
    ContractInfo {
        version: c.version as u8,
        genesis,
        schema_id,
        terminals,
        witnesses,
        bundle_count,
        script_count,
        types_count,
    }
}

fn kit_info(k: &Kit) -> KitInfo {
    KitInfo {
        version: k.version as u8,
        schema_count: k.schemata.len() as u32,
        script_count: k.scripts.len() as u32,
        types_count: k.types.len() as u32,
    }
}

fn consignment_common<const TRANSFER: bool>(
    c: &Consignment<TRANSFER>,
) -> (
    GenesisInfo,
    String,
    Vec<TerminalInfo>,
    Vec<WitnessInfo>,
    u32,
    u32,
    u32,
) {
    let schema_id = c.schema.schema_id().to_string();
    let genesis = genesis_info(c);

    let terminals = c
        .terminals
        .iter()
        .map(|(bundle_id, secret_seals)| TerminalInfo {
            bundle_id: bundle_id.to_string(),
            secret_seal_count: secret_seals.len() as u32,
        })
        .collect();

    let witnesses = c
        .bundles
        .iter()
        .map(|wb| {
            let bundle = wb.bundle();
            let transitions = bundle
                .known_transitions
                .iter()
                .map(transition_info)
                .collect();
            WitnessInfo {
                txid: wb.witness_id().to_string(),
                transition_count: bundle.known_transitions.len() as u32,
                input_count: bundle.input_map.len() as u32,
                transitions,
            }
        })
        .collect();

    let bundle_count = c.bundles.len() as u32;
    let script_count = c.scripts.len() as u32;
    let types_count = c.types.len() as u32;

    (
        genesis,
        schema_id,
        terminals,
        witnesses,
        bundle_count,
        script_count,
        types_count,
    )
}

fn genesis_info<const TRANSFER: bool>(c: &Consignment<TRANSFER>) -> GenesisInfo {
    let g = &c.genesis;

    let mut asset = AssetMeta::default();
    asset.extract_from(c);

    GenesisInfo {
        contract_id: c.contract_id().to_string(),
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
    }
}

/// Decoded asset metadata pulled from genesis global state.
/// `None` means the schema doesn't carry the field, the data
/// was malformed, or the type system couldn't decode it.
#[derive(Default)]
struct AssetMeta {
    name: Option<String>,
    ticker: Option<String>,
    precision: Option<u8>,
    details: Option<String>,
}

impl AssetMeta {
    fn extract_from<const TRANSFER: bool>(&mut self, c: &Consignment<TRANSFER>) {
        // Try the AssetSpec composite first (NIA/IFA/PFA/UDA). It carries
        // ticker, name, precision and optional details in a single value.
        if let Some((spec, sem_id)) = decode_global(c, GlobalStateType::with(GS_NOMINAL)) {
            self.absorb_asset_spec(&c.types, &spec, sem_id);
        }

        // Fall back to / overlay scalar fields used by schemas like CFA.
        if self.name.is_none()
            && let Some((v, _)) = decode_global(c, GlobalStateType::with(GS_NAME))
        {
            self.name = strict_val_to_string(&v);
        }

        if self.precision.is_none()
            && let Some((v, sem_id)) = decode_global(c, GlobalStateType::with(GS_PRECISION))
        {
            self.precision = enum_val_to_ord(&c.types, &v, sem_id);
        }

        if self.details.is_none()
            && let Some((v, _)) = decode_global(c, GlobalStateType::with(GS_DETAILS))
        {
            self.details = strict_val_to_string(&v);
        }
    }

    fn absorb_asset_spec(&mut self, types: &TypeSystem, spec: &StrictVal, sem_id: SemId) {
        let StrictVal::Struct(fields) = spec.skip_wrapper() else {
            return;
        };

        let struct_def = match types.get(sem_id) {
            Some(Ty::Struct(s)) => Some(s),
            _ => None,
        };

        let field_sem_id = |name: &'static str| -> Option<SemId> {
            struct_def
                .and_then(|s| s.ty_by_name(&FieldName::from(name)))
                .copied()
        };

        if let Some(v) = fields.get(&FieldName::from("ticker")) {
            self.ticker = strict_val_to_string(v);
        }

        if let Some(v) = fields.get(&FieldName::from("name")) {
            self.name = strict_val_to_string(v);
        }

        if let Some(v) = fields.get(&FieldName::from("precision"))
            && let Some(precision_sem_id) = field_sem_id("precision")
        {
            self.precision = enum_val_to_ord(types, v, precision_sem_id);
        }

        if let Some(v) = fields.get(&FieldName::from("details")) {
            self.details = strict_val_to_opt_string(v);
        }
    }
}

fn decode_global<const TRANSFER: bool>(
    c: &Consignment<TRANSFER>,
    type_id: GlobalStateType,
) -> Option<(StrictVal, SemId)> {
    let details = c.schema.global_types.get(&type_id)?;
    let values = c.genesis.globals.get(&type_id)?;
    let first = values.first()?;
    let sem_id = details.global_state_schema.sem_id;

    let val = c
        .types
        .strict_deserialize_type(sem_id, first.as_slice())
        .ok()?
        .unbox();

    Some((val, sem_id))
}

fn strict_val_to_string(v: &StrictVal) -> Option<String> {
    match v.skip_wrapper() {
        StrictVal::String(s) => Some(s.clone()),
        StrictVal::Bytes(b) => String::from_utf8(b.to_inner()).ok(),
        StrictVal::List(items) if items.is_empty() => Some(String::new()),
        StrictVal::List(items) if items.iter().all(|c| matches!(c, StrictVal::Enum(_))) => {
            let bytes: Vec<u8> = items
                .iter()
                .filter_map(|c| match c {
                    StrictVal::Enum(EnumTag::Ord(o)) => Some(*o),
                    _ => None,
                })
                .collect();
            String::from_utf8(bytes).ok()
        }
        _ => None,
    }
}

fn strict_val_to_opt_string(v: &StrictVal) -> Option<String> {
    let StrictVal::Union(tag, content) = v.skip_wrapper() else {
        return None;
    };
    let is_some =
        matches!(tag, EnumTag::Name(n) if n.as_str() == "some") || matches!(tag, EnumTag::Ord(1));
    if !is_some {
        return None;
    }

    strict_val_to_string(content)
}

fn enum_val_to_ord(types: &TypeSystem, v: &StrictVal, sem_id: SemId) -> Option<u8> {
    match v.skip_wrapper() {
        StrictVal::Enum(EnumTag::Ord(o)) => Some(*o),
        StrictVal::Enum(EnumTag::Name(name)) => match types.get(sem_id)? {
            Ty::Enum(variants) => variants.tag_by_name(name),
            _ => resolve_variant_name(types, sem_id, name),
        },
        StrictVal::Number(n) => u8::try_from(n.unwrap_uint::<u64>()).ok(),
        _ => None,
    }
}

fn resolve_variant_name(types: &TypeSystem, sem_id: SemId, name: &VariantName) -> Option<u8> {
    match types.get(sem_id)? {
        Ty::Tuple(fields) if fields.len() == 1 => {
            let inner = fields.iter().next()?;
            resolve_variant_name(types, *inner, name)
        }
        Ty::Enum(variants) => variants.tag_by_name(name),
        _ => None,
    }
}

fn transition_info(kt: &KnownTransition) -> TransitionInfo {
    let t: &Transition = &kt.transition;
    TransitionInfo {
        op_id: kt.opid.to_string(),
        transition_type: t.transition_type.to_inner(),
        input_count: t.inputs.len() as u32,
        fungible_allocations: fungible_allocations(&t.assignments),
    }
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

fn fungible_entry<Seal: ExposedSeal>(a: &Assign<rgbstd::RevealedValue, Seal>) -> FungibleEntry {
    let amount: u64 = (*a.as_revealed_state().as_inner()).into();
    let seal = match a {
        Assign::Revealed { seal, .. } => SealInfo::Revealed {
            txid: seal.txid().map(|t| t.to_string()),
            vout: seal.vout().to_u32(),
        },
        Assign::ConfidentialSeal { seal, .. } => SealInfo::Confidential {
            secret_seal: seal.to_string(),
        },
    };

    FungibleEntry { amount, seal }
}
