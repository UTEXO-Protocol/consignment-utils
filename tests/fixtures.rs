//! End-to-end tests over V1 consignment files.
//!
//! `synthetic` builds every fixture in `testdata/` except the legacy ones, with
//! the rgb-schemas and rgb-ops revs rgb-lib f3975623 uses. No release-produced
//! transfer exists yet, so the BFA transfer is synthetic.

mod synthetic;

use std::collections::BTreeSet;
use std::ffi::{CStr, c_char};
use std::path::PathBuf;
use std::ptr;

use amplify::Wrapper;
use rgb_consignment::{
    ConsignmentInfo, ConsignmentKind, ContractInfo, GlobalEntry, MetaEntry, ParseError, SealInfo,
    TransferInfo, parse,
};
use rgbstd::rgbcore::validation::Status;
use schemata::BFA_SCHEMA_ID;

const BFA_CONTRACT: &[u8] = include_bytes!("../testdata/bfa-contract.rgb");
const IFA_CONTRACT: &[u8] = include_bytes!("../testdata/ifa-contract.rgb");
const CFA_CONTRACT: &[u8] = include_bytes!("../testdata/cfa-contract.rgb");
const LEGACY_V0_BURN: &[u8] = include_bytes!("../testdata/legacy-v0-bfa-burn.rgb");
const LEGACY_TERMINALS_CONTRACT: &[u8] =
    include_bytes!("../testdata/legacy-terminals-bfa-contract.rgb");
const LEGACY_TERMINALS_TRANSFER: &[u8] =
    include_bytes!("../testdata/legacy-terminals-bfa-transfer.rgb");

// The BFA ids bridge-utexo and fsn hardcode, tied to rgb-schemas below.
const TS_BURN: u16 = 8010;
const TS_MINT: u16 = 8013;
const TS_PROPAGATE: u16 = 8014;
const TS_TRANSFER: u16 = 10000;
const OS_ASSET: u16 = 4000;
const GS_ISSUED_SUPPLY: u16 = 2010;
const GS_BURNED_ASSET: u16 = 2015;
const GS_BRIDGE_LOCATION: u16 = 3008;
const GS_BURN_REASON: u16 = 3009;
const MS_BEFORE_BLOCK: u16 = 1003;

#[test]
fn consumer_ids_match_rgb_schemas() {
    let transitions = [
        schemata::TS_BURN,
        schemata::TS_MINT,
        schemata::TS_PROPAGATE,
        schemata::TS_TRANSFER,
    ];
    assert_eq!(
        transitions.map(|ty| ty.to_inner()),
        [TS_BURN, TS_MINT, TS_PROPAGATE, TS_TRANSFER]
    );
    let globals = [
        schemata::GS_ISSUED_SUPPLY,
        schemata::GS_BURNED_ASSET,
        schemata::GS_BRIDGE_LOCATION,
        schemata::GS_BURN_REASON,
    ];
    assert_eq!(
        globals.map(|ty| ty.to_inner()),
        [
            GS_ISSUED_SUPPLY,
            GS_BURNED_ASSET,
            GS_BRIDGE_LOCATION,
            GS_BURN_REASON
        ]
    );
    assert_eq!(schemata::OS_ASSET.to_inner(), OS_ASSET);
    assert_eq!(schemata::MS_BEFORE_BLOCK.to_inner(), MS_BEFORE_BLOCK);
}

unsafe extern "C" {
    fn rgb_consignment_parse(data: *const u8, len: usize, err_out: *mut *mut c_char)
    -> *mut c_char;
    fn rgb_consignment_string_free(s: *mut c_char);
}

fn contract(bytes: &[u8]) -> ContractInfo {
    match parse(bytes).expect("fixture must parse") {
        ConsignmentInfo::Contract(c) => c,
        other => panic!("expected contract, got {:?}", other.kind()),
    }
}

fn transfer(bytes: &[u8]) -> TransferInfo {
    match parse(bytes).expect("transfer must parse") {
        ConsignmentInfo::Transfer(t) => t,
        other => panic!("expected transfer, got {:?}", other.kind()),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn global(global_type: u16, value: &[u8]) -> GlobalEntry {
    GlobalEntry {
        global_type,
        value_hex: hex(value),
    }
}

fn synthetic_transfer() -> (synthetic::Built, TransferInfo) {
    let built = synthetic::build();
    let info = transfer(&synthetic::file_bytes(&built.transfer));
    (built, info)
}

#[test]
fn bfa_contract_reports_root_level_nominals() {
    let c = contract(BFA_CONTRACT);
    assert_eq!(c.version, 1);
    let g = &c.genesis;
    assert_eq!(
        g.contract_id,
        "rgb:IP7rzsJ3-KTivsnR-lwYBS0E-dIKpPU8-OayQsVt-JI~hdCA"
    );
    assert_eq!(g.schema_id, BFA_SCHEMA_ID.to_string());
    assert_eq!(c.schema_id, g.schema_id);
    assert_eq!(g.chain_net, "tb4");
    assert_eq!(g.name.as_deref(), Some("Test asset"));
    assert_eq!(g.ticker.as_deref(), Some(synthetic::BFA_TICKER));
    assert_eq!(g.details.as_deref(), Some(synthetic::BFA_DETAILS));
    assert_eq!(g.precision, Some(8));
    // Mint rights are declarative: genesis allocates no asset.
    assert!(g.fungible_allocations.is_empty());
    assert_eq!(g.assignment_count, 1);
    assert!(c.witnesses.is_empty());
}

#[test]
fn bfa_genesis_exposes_bridge_location() {
    let g = contract(BFA_CONTRACT).genesis;
    // BridgeLocation::Evm: union tag 0, chain id 1 (u64 LE), 20-byte address.
    let location = format!("00{}{}", hex(&1u64.to_le_bytes()), "deadbeef".repeat(5));
    let entry = g
        .globals
        .iter()
        .find(|e| e.global_type == GS_BRIDGE_LOCATION);
    assert_eq!(entry.map(|e| e.value_hex.as_str()), Some(location.as_str()));
    // The count is of global types; globals has one entry per value.
    let types: BTreeSet<u16> = g.globals.iter().map(|e| e.global_type).collect();
    assert_eq!(types.len() as u32, g.global_state_count);
}

#[test]
fn ifa_contract_exposes_genesis_allocations() {
    let g = contract(IFA_CONTRACT).genesis;
    assert_eq!(g.ticker.as_deref(), Some("TEST"));
    assert_eq!(g.name.as_deref(), Some("Test asset"));
    assert_eq!(g.fungible_allocations.len(), 2);
    let a = &g.fungible_allocations[0];
    assert_eq!(
        (a.assignment_type, a.total, a.entries.len()),
        (4000, synthetic::ISSUED, 1)
    );
    match &a.entries[0].seal {
        SealInfo::Revealed { txid, vout } => {
            assert_eq!(
                txid.as_deref(),
                Some("14295d5bb1a191cdb6286dc0944df938421e3dfcbf0811353ccac4100c2068c5")
            );
            assert_eq!(*vout, 1);
        }
        other => panic!("unexpected seal {other:?}"),
    }
    let inflation = &g.fungible_allocations[1];
    assert_eq!((inflation.assignment_type, inflation.total), (4010, 50_000));
}

#[test]
fn cfa_contract_decodes_scalar_asset_globals() {
    let g = contract(CFA_CONTRACT).genesis;
    assert_eq!(g.name.as_deref(), Some("Test asset"));
    assert_eq!(g.ticker, None);
    assert_eq!(g.precision, Some(8));
}

#[test]
fn contract_via_c_abi() {
    let mut err = ptr::null_mut();
    let json =
        unsafe { rgb_consignment_parse(BFA_CONTRACT.as_ptr(), BFA_CONTRACT.len(), &mut err) };
    assert!(err.is_null(), "unexpected error: {}", unsafe {
        CStr::from_ptr(err).to_string_lossy()
    });
    assert!(!json.is_null());

    let text = unsafe { CStr::from_ptr(json) }.to_str().unwrap().to_owned();
    unsafe { rgb_consignment_string_free(json) };

    // JSON produced by the C ABI must round-trip into the public Rust type.
    let info: ConsignmentInfo = serde_json::from_str(&text).unwrap();
    assert_eq!(info.kind(), ConsignmentKind::Contract);
    assert!(text.contains(r#""global_type":3008"#), "{text}");
    assert!(!text.contains("terminals"), "{text}");
}

#[test]
fn truncated_fixture_is_a_decode_error_not_a_legacy_layout() {
    let err = parse(&BFA_CONTRACT[..BFA_CONTRACT.len() / 2]).unwrap_err();
    assert!(matches!(err, ParseError::Decode(_)), "{err}");
}

#[test]
fn container_type_must_match_transfer_flag() {
    let mut bytes = BFA_CONTRACT.to_vec();
    bytes[4..7].copy_from_slice(b"VTF");
    let err = parse(&bytes).unwrap_err();
    assert!(err.to_string().contains("transfer flag"), "{err}");
}

// The hard cutover relies on this: pre-cutover BFA consignments must not parse.
#[test]
fn legacy_v0_transfer_is_rejected() {
    assert_eq!(&LEGACY_V0_BURN[..8], b"RGB\x00TFR\x00");
    let err = parse(LEGACY_V0_BURN).unwrap_err();
    assert!(err.to_string().contains("ConsignmentVer"), "{err}");
}

#[test]
fn terminals_layout_contract_is_rejected_as_legacy() {
    let err = parse(LEGACY_TERMINALS_CONTRACT).unwrap_err();
    assert!(matches!(err, ParseError::LegacyTerminals), "{err}");
}

#[test]
fn terminals_layout_transfer_is_rejected_as_legacy() {
    let err = parse(LEGACY_TERMINALS_TRANSFER).unwrap_err();
    assert!(matches!(err, ParseError::LegacyTerminals), "{err}");
    assert!(err.to_string().contains("terminals"), "{err}");
}

#[test]
fn validated_transfer_container_parses_like_plain_one() {
    let built = synthetic::build();
    let plain = synthetic::file_bytes(&built.transfer);
    let validated = synthetic::validated_file_bytes(&built.transfer, &Status::default());
    assert_eq!(&validated[..7], b"RGB\x00VTF");
    assert_eq!(
        serde_json::to_value(transfer(&validated)).unwrap(),
        serde_json::to_value(transfer(&plain)).unwrap()
    );
}

#[test]
fn plain_contract_container_parses_like_validated_one() {
    let bytes = synthetic::file_bytes(&synthetic::bfa_contract().into_consignment());
    assert_eq!(&bytes[..7], b"RGB\x00CON");
    assert_eq!(
        serde_json::to_value(contract(&bytes)).unwrap(),
        serde_json::to_value(contract(BFA_CONTRACT)).unwrap()
    );
}

#[test]
fn synthetic_transfer_lists_witnesses_in_consignment_order() {
    let (built, t) = synthetic_transfer();
    assert_eq!((t.version, t.bundle_count), (1, 3));
    assert_eq!(
        t.genesis.contract_id,
        contract(BFA_CONTRACT).genesis.contract_id
    );
    let txids: Vec<String> = built
        .transfer
        .bundles
        .iter()
        .map(|wb| wb.witness_id().to_string())
        .collect();
    let parsed: Vec<&str> = t.witnesses.iter().map(|w| w.txid.as_str()).collect();
    assert_eq!(parsed, txids);
    let ops: Vec<Vec<String>> = t
        .witnesses
        .iter()
        .map(|w| w.transitions.iter().map(|tr| tr.op_id.clone()).collect())
        .collect();
    let want = [
        vec![built.propagate],
        vec![built.mint],
        vec![built.split, built.burn],
    ];
    let want: Vec<Vec<String>> = want
        .iter()
        .map(|ids| ids.iter().map(ToString::to_string).collect())
        .collect();
    assert_eq!(ops, want);
}

#[test]
fn propagate_transition_is_reported_by_its_type_number() {
    let (_, t) = synthetic_transfer();
    let propagate = &t.witnesses[0].transitions[0];
    assert_eq!(propagate.transition_type, TS_PROPAGATE);
    assert_eq!(propagate.input_count, 1);
    // Mint rights are declarative, so nothing fungible to report.
    assert!(propagate.fungible_allocations.is_empty());
}

#[test]
fn synthetic_mint_exposes_issued_supply_and_before_block() {
    let (_, t) = synthetic_transfer();
    let mint = &t.witnesses[1].transitions[0];
    assert_eq!(mint.transition_type, TS_MINT);
    assert_eq!(
        mint.globals,
        vec![global(GS_ISSUED_SUPPLY, &synthetic::MINTED.to_le_bytes())]
    );
    assert_eq!(
        mint.metadata,
        vec![MetaEntry {
            meta_type: MS_BEFORE_BLOCK,
            value_hex: hex(&synthetic::BEFORE_BLOCK.to_le_bytes()),
        }]
    );
    let allocation = &mint.fungible_allocations[0];
    assert_eq!(
        (allocation.assignment_type, allocation.total),
        (OS_ASSET, synthetic::MINTED)
    );
    assert!(matches!(
        allocation.entries[0].seal,
        SealInfo::Revealed {
            txid: None,
            vout: 0
        }
    ));
}

#[test]
fn burn_witness_holds_the_split_then_the_burn() {
    let (_, t) = synthetic_transfer();
    let types: Vec<Vec<u16>> = t
        .witnesses
        .iter()
        .map(|w| w.transitions.iter().map(|tr| tr.transition_type).collect())
        .collect();
    assert_eq!(
        types,
        vec![
            vec![TS_PROPAGATE],
            vec![TS_MINT],
            vec![TS_TRANSFER, TS_BURN],
        ]
    );
    // The input map spends the mint output and, inside the bundle, the split output.
    assert_eq!(t.witnesses[2].input_count, 2);
}

#[test]
fn split_sends_change_and_puts_the_burned_part_on_the_spent_outpoint() {
    let (_, t) = synthetic_transfer();
    let burn_witness = &t.witnesses[2];
    let split = &burn_witness.transitions[0];
    assert_eq!(split.input_count, 1);
    let mut entries: Vec<(u64, SealInfo)> = split.fungible_allocations[0]
        .entries
        .iter()
        .map(|e| (e.amount, e.seal.clone()))
        .collect();
    entries.sort_by_key(|(amount, _)| *amount);
    let [(burned, on_spent), (change, change_seal)] = entries.as_slice() else {
        panic!("split must have two outputs: {entries:?}");
    };
    assert_eq!(
        (*burned, *change),
        (synthetic::BURNED, synthetic::MINTED - synthetic::BURNED)
    );
    // The burn tx spends the mint output, and the split assigns to that outpoint.
    let mint_txid = &t.witnesses[1].txid;
    assert!(
        matches!(on_spent, SealInfo::Revealed { txid: Some(txid), vout: 0 } if txid == mint_txid),
        "{on_spent:?}"
    );
    assert!(
        matches!(
            change_seal,
            SealInfo::Revealed {
                txid: None,
                vout: 1
            }
        ),
        "{change_seal:?}"
    );
}

#[test]
fn burn_has_no_change_and_exposes_burned_asset_and_reason() {
    let (_, t) = synthetic_transfer();
    let burn = &t.witnesses[2].transitions[1];
    assert_eq!(burn.transition_type, TS_BURN);
    assert_eq!(burn.input_count, 1);
    assert!(burn.fungible_allocations.is_empty());
    assert!(burn.metadata.is_empty());
    // Burn reason is a bare 32-byte array: no length prefix.
    assert_eq!(
        burn.globals,
        vec![
            global(GS_BURNED_ASSET, &synthetic::BURNED.to_le_bytes()),
            global(GS_BURN_REASON, &synthetic::burn_reason()),
        ]
    );
}

fn testdata(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(name)
}

fn built_fixtures() -> [(&'static str, Vec<u8>); 4] {
    [
        (
            "bfa-contract.rgb",
            synthetic::contract_file_bytes(&synthetic::bfa_contract()),
        ),
        (
            "ifa-contract.rgb",
            synthetic::contract_file_bytes(&synthetic::ifa_contract()),
        ),
        (
            "cfa-contract.rgb",
            synthetic::contract_file_bytes(&synthetic::cfa_contract()),
        ),
        (
            "bfa-transfer-synthetic.rgb",
            synthetic::file_bytes(&synthetic::build().transfer),
        ),
    ]
}

#[test]
fn committed_fixtures_match_builder() {
    for (name, bytes) in built_fixtures() {
        let committed = std::fs::read(testdata(name)).expect("run regenerate_fixtures");
        assert!(
            committed == bytes,
            "{name} is stale: run regenerate_fixtures"
        );
    }
}

#[test]
#[ignore = "writes the built testdata/ fixtures the Go tests read; commit the result"]
fn regenerate_fixtures() {
    for (name, bytes) in built_fixtures() {
        std::fs::write(testdata(name), bytes).unwrap();
    }
}
