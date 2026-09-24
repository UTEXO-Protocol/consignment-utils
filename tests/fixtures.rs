//! End-to-end tests over real consignment files in `testdata/`.

use std::ffi::{CStr, c_char};
use std::ptr;

use rgb_consignment::{ConsignmentInfo, ConsignmentKind, SealInfo, parse};

const IFA_CONTRACT: &[u8] = include_bytes!("../testdata/ifa-contract.rgb");

unsafe extern "C" {
    fn rgb_consignment_parse(data: *const u8, len: usize, err_out: *mut *mut c_char) -> *mut c_char;
    fn rgb_consignment_string_free(s: *mut c_char);
}

#[test]
fn ifa_contract_parses() {
    let info = parse(IFA_CONTRACT).expect("fixture must parse");
    assert_eq!(info.kind(), ConsignmentKind::Contract);

    let ConsignmentInfo::Contract(c) = info else {
        panic!("expected contract");
    };
    let g = &c.genesis;
    assert_eq!(g.contract_id, "rgb:jpyfP_3m-zroPnJm-2J9qFHO-7ZjimUC-yH4A96u-oHthc64");
    assert_eq!(g.chain_net, "tb4");
    assert_eq!(g.timestamp, 1768417034);
    assert_eq!(g.name.as_deref(), Some("Test asset"));
    assert_eq!(g.ticker.as_deref(), Some("TEST"));
    assert_eq!(g.precision, Some(8));
    assert_eq!(g.details, None);
    assert_eq!(g.global_state_count, 5);
    assert_eq!(g.assignment_count, 2);

    assert_eq!(g.fungible_allocations.len(), 2);
    let a = &g.fungible_allocations[0];
    assert_eq!(a.assignment_type, 4000);
    assert_eq!(a.total, 100_000);
    assert_eq!(a.entries.len(), 1);
    match &a.entries[0].seal {
        SealInfo::Revealed { txid, vout } => {
            assert_eq!(txid.as_deref(), Some("14295d5bb1a191cdb6286dc0944df938421e3dfcbf0811353ccac4100c2068c5"));
            assert_eq!(*vout, 1);
        }
        other => panic!("unexpected seal {other:?}"),
    }
    assert_eq!(g.fungible_allocations[1].assignment_type, 4010);
    assert_eq!(g.fungible_allocations[1].total, 50_000);

    assert_eq!(c.schema_id, g.schema_id);
    assert!(c.terminals.is_empty());
    assert!(c.witnesses.is_empty());
    assert_eq!(c.bundle_count, 0);
    assert_eq!(c.script_count, 3);
    assert_eq!(c.types_count, 39);
}

#[test]
fn ifa_contract_via_c_abi() {
    let mut err = ptr::null_mut();
    let json = unsafe { rgb_consignment_parse(IFA_CONTRACT.as_ptr(), IFA_CONTRACT.len(), &mut err) };
    assert!(err.is_null(), "unexpected error: {}", unsafe { CStr::from_ptr(err) }.to_string_lossy());
    assert!(!json.is_null());

    let text = unsafe { CStr::from_ptr(json) }.to_str().unwrap().to_owned();
    unsafe { rgb_consignment_string_free(json) };

    // JSON produced by the C ABI must round-trip into the public Rust type.
    let info: ConsignmentInfo = serde_json::from_str(&text).unwrap();
    assert_eq!(info.kind(), ConsignmentKind::Contract);
    assert!(text.contains(r#""ticker":"TEST""#), "{text}");
}

#[test]
fn truncated_fixture_is_rejected() {
    let cut = &IFA_CONTRACT[..IFA_CONTRACT.len() / 2];
    assert!(parse(cut).is_err());
}
