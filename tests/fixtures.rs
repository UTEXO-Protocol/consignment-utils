//! End-to-end tests over V1 consignment files.
//!
//! The contracts in `testdata/` are the rgb-schemas 0.11.2-rc.1 examples. No
//! release-produced transfer exists yet, so `synthetic` builds one with the
//! rc.1 types and writes it the way rgb-lib does.

mod synthetic;

use std::collections::BTreeSet;
use std::ffi::{CStr, c_char};
use std::path::PathBuf;
use std::ptr;

use rgb_consignment::{
    ConsignmentInfo, ConsignmentKind, ContractInfo, GlobalEntry, MetaEntry, SealInfo, TransferInfo,
    TransitionInfo, parse,
};

const BFA_CONTRACT: &[u8] = include_bytes!("../testdata/bfa-contract.rgb");
const IFA_CONTRACT: &[u8] = include_bytes!("../testdata/ifa-contract.rgb");
const CFA_CONTRACT: &[u8] = include_bytes!("../testdata/cfa-contract.rgb");
const LEGACY_V0_BURN: &[u8] = include_bytes!("../testdata/legacy-v0-bfa-burn.rgb");
const SYNTHETIC_TRANSFER: &str = "testdata/bfa-transfer-synthetic.rgb";

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

#[test]
fn bfa_contract_parses() {
    let c = contract(BFA_CONTRACT);
    assert_eq!(c.version, 1);
    let g = &c.genesis;
    assert_eq!(
        g.contract_id,
        "rgb:UiDQRpwu-~dsy7ZH-raGXsnJ-UNiT13f-WbArf5m-jPeb8xU"
    );
    assert_eq!(
        g.schema_id,
        "rgb:sch:Ryl5eQZ3IHrU05e3AWdQZJR4FB5GvxZPy96cwhJiApc#amazon-norway-harris"
    );
    assert_eq!(c.schema_id, g.schema_id);
    assert_eq!(g.chain_net, "tb4");
    assert_eq!(g.name.as_deref(), Some("Test asset"));
    assert_eq!(g.ticker.as_deref(), Some("TEST"));
    assert_eq!(g.precision, Some(8));
    // Mint rights are declarative: genesis allocates no asset.
    assert!(g.fungible_allocations.is_empty());
    assert_eq!(g.assignment_count, 1);
    assert!(c.terminals.is_empty() && c.witnesses.is_empty());
}

#[test]
fn bfa_genesis_exposes_bridge_location() {
    let g = contract(BFA_CONTRACT).genesis;
    // BridgeLocation::Evm: union tag 0, chain id 1 (u64 LE), 20-byte address.
    let location = format!("00{}{}", hex(&1u64.to_le_bytes()), "deadbeef".repeat(5));
    let entry = g.globals.iter().find(|e| e.global_type == 3008);
    assert_eq!(entry.map(|e| e.value_hex.as_str()), Some(location.as_str()));
    // The count is of global types; globals has one entry per value.
    let types: BTreeSet<u16> = g.globals.iter().map(|e| e.global_type).collect();
    assert_eq!(types.len() as u32, g.global_state_count);
}

#[test]
fn ifa_contract_exposes_genesis_allocations() {
    let g = contract(IFA_CONTRACT).genesis;
    assert_eq!(g.ticker.as_deref(), Some("TEST"));
    assert_eq!(g.fungible_allocations.len(), 2);
    let a = &g.fungible_allocations[0];
    assert_eq!(
        (a.assignment_type, a.total, a.entries.len()),
        (4000, 100_000, 1)
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
}

#[test]
fn truncated_fixture_is_rejected() {
    assert!(parse(&BFA_CONTRACT[..BFA_CONTRACT.len() / 2]).is_err());
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
fn validated_transfer_container_parses_like_plain_one() {
    let built = synthetic::build(synthetic::bfa_genesis(BFA_CONTRACT));
    let plain = synthetic::file_bytes(&built.transfer);
    let validated = synthetic::validated_file_bytes(&built.transfer);
    assert_eq!(&validated[..7], b"RGB\x00VTF");
    assert_eq!(
        serde_json::to_value(transfer(&validated)).unwrap(),
        serde_json::to_value(transfer(&plain)).unwrap()
    );
}

#[test]
fn plain_contract_container_parses_like_validated_one() {
    let built = synthetic::build(synthetic::bfa_genesis(BFA_CONTRACT));
    let bytes = synthetic::file_bytes(&built.transfer.into_contract());
    assert_eq!(&bytes[..7], b"RGB\x00CON");
    assert_eq!(
        contract(&bytes).genesis.contract_id,
        contract(BFA_CONTRACT).genesis.contract_id
    );
}

fn mint_of(t: &TransferInfo) -> &TransitionInfo {
    &t.witnesses[0].transitions[0]
}

fn burn_of(t: &TransferInfo) -> &TransitionInfo {
    &t.witnesses[1].transitions[0]
}

#[test]
fn synthetic_transfer_lists_both_witnesses() {
    let built = synthetic::build(synthetic::bfa_genesis(BFA_CONTRACT));
    let t = transfer(&synthetic::file_bytes(&built.transfer));
    assert_eq!(t.version, 1);
    assert_eq!(t.bundle_count, 2);
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
    assert_eq!(mint_of(&t).op_id, built.mint.to_string());
    assert_eq!(burn_of(&t).op_id, built.burn.to_string());
}

#[test]
fn synthetic_mint_exposes_issued_supply_and_after_block() {
    let built = synthetic::build(synthetic::bfa_genesis(BFA_CONTRACT));
    let t = transfer(&synthetic::file_bytes(&built.transfer));
    let mint = mint_of(&t);
    assert_eq!(mint.transition_type, synthetic::TS_MINT);
    assert_eq!(
        mint.globals,
        vec![global(
            synthetic::GS_ISSUED_SUPPLY,
            &synthetic::MINTED.to_le_bytes()
        )]
    );
    assert_eq!(
        mint.metadata,
        vec![MetaEntry {
            meta_type: synthetic::MS_AFTER_BLOCK,
            value_hex: hex(&synthetic::AFTER_BLOCK.to_le_bytes()),
        }]
    );
    let allocation = &mint.fungible_allocations[0];
    assert_eq!(
        (allocation.assignment_type, allocation.total),
        (synthetic::OS_ASSET, synthetic::MINTED)
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
fn synthetic_burn_exposes_burned_asset_and_reason() {
    let built = synthetic::build(synthetic::bfa_genesis(BFA_CONTRACT));
    let t = transfer(&synthetic::file_bytes(&built.transfer));
    let burn = burn_of(&t);
    assert_eq!(burn.transition_type, synthetic::TS_BURN);
    assert!(burn.metadata.is_empty());
    // Burn reason is a bare 32-byte array: no length prefix.
    assert_eq!(
        burn.globals,
        vec![
            global(synthetic::GS_BURNED_ASSET, &synthetic::BURNED.to_le_bytes()),
            global(synthetic::GS_BURN_REASON, &synthetic::burn_reason()),
        ]
    );
    let change = &burn.fungible_allocations[0].entries[0];
    assert_eq!(change.amount, synthetic::MINTED - synthetic::BURNED);
    let SealInfo::Confidential { secret_seal } = &change.seal else {
        panic!("change seal should be concealed: {:?}", change.seal);
    };
    assert_eq!(*secret_seal, built.change_seal.to_secret_seal().to_string());
}

#[test]
fn synthetic_terminals_list_concealed_and_revealed_seals() {
    let built = synthetic::build(synthetic::bfa_genesis(BFA_CONTRACT));
    let burn_bundle = built.transfer.bundles[1].bundle.bundle_id().to_string();
    let t = transfer(&synthetic::file_bytes(&built.transfer));
    assert_eq!(t.terminals.len(), 1);
    assert_eq!(t.terminals[0].bundle_id, burn_bundle);
    let [concealed, revealed] = t.terminals[0].seals.as_slice() else {
        panic!("expected two terminal seals: {:?}", t.terminals[0].seals);
    };
    assert!(
        matches!(concealed, SealInfo::Confidential { secret_seal } if *secret_seal == built.change_seal.to_secret_seal().to_string())
    );
    assert!(matches!(
        revealed,
        SealInfo::Revealed {
            txid: None,
            vout: 2
        }
    ));
    // A witness-vout terminal resolves through the witness of the same bundle.
    let witness = t.witnesses.iter().find(|w| w.bundle_id == burn_bundle);
    assert_eq!(
        witness.map(|w| w.txid.clone()),
        Some(built.transfer.bundles[1].witness_id().to_string())
    );
}

fn synthetic_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SYNTHETIC_TRANSFER)
}

#[test]
fn committed_synthetic_transfer_matches_builder() {
    let built = synthetic::build(synthetic::bfa_genesis(BFA_CONTRACT));
    let committed = std::fs::read(synthetic_path()).expect("run regenerate_synthetic_transfer");
    assert_eq!(committed, synthetic::file_bytes(&built.transfer));
}

#[test]
#[ignore = "writes testdata/bfa-transfer-synthetic.rgb for the Go tests; commit the result"]
fn regenerate_synthetic_transfer() {
    let built = synthetic::build(synthetic::bfa_genesis(BFA_CONTRACT));
    std::fs::write(synthetic_path(), synthetic::file_bytes(&built.transfer)).unwrap();
}
