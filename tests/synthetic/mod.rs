//! Builds the fixtures with the rgb-schemas and rgb-ops revs rgb-lib f3975623 uses.
//!
//! The contracts are issued (and validated) with the inputs of the rgb-schemas
//! examples. The BFA transfer is well formed, but its anchors are dummies, so
//! it would never validate.

use std::collections::BTreeMap;
use std::str::FromStr;

use amplify::Bytes32;
use amplify::confinement::{LargeVec, NonEmptyOrdMap, NonEmptyVec};
use rgbstd::bitcoin::absolute::LockTime;
use rgbstd::bitcoin::transaction::Version;
use rgbstd::bitcoin::{OutPoint, Transaction, TxIn};
use rgbstd::containers::{ConsignmentVer, FileContent, Transfer, ValidContract, WitnessBundle};
use rgbstd::contract::{AllocatedState, ContractBuilder, IssuerWrapper, TransitionBuilder};
use rgbstd::rgbcore::validation::Status;
use rgbstd::stl::{
    AssetSpec, BlockNumber, BridgeLocation, BurnReason, ContractTerms, Details, EvmAddress,
    EvmContract, Name, RejectListLocation, RejectListUrl, RicardianContract, UnicodeTicker,
};
use rgbstd::{
    Amount, ChainNet, ContractId, GenesisSeal, GraphSeal, KnownTransition, OpId, Operation, Opout,
    Precision, Transition, TransitionBundle, TransitionType, Txid,
};
use schemata::{
    BridgedFungibleAsset, CollectibleFungibleAsset, InflatableFungibleAsset, OS_ASSET, OS_MINT,
    TS_BURN, TS_MINT, TS_PROPAGATE, TS_TRANSFER,
};
use strict_types::encoding::StrictDumb;

/// rgb-schemas `examples/common`: the issuance time and the beneficiary tx.
const CREATED_AT: i64 = 1713261744;
const BENEFICIARY_TXID: &str = "14295d5bb1a191cdb6286dc0944df938421e3dfcbf0811353ccac4100c2068c5";

pub const ISSUED: u64 = 100_000;
pub const MAX_SUPPLY: u64 = 150_000;
pub const MINTED: u64 = 1000;
pub const BURNED: u64 = 400;
pub const BEFORE_BLOCK: u64 = 42;
pub const BFA_TICKER: &str = "₮EST";
pub const BFA_DETAILS: &str = "Bridged test asset";

/// 12 zero bytes + a 20-byte EVM recipient, the bridge's burn reason layout.
pub fn burn_reason() -> [u8; 32] {
    let mut reason = [0xab; 32];
    reason[..12].fill(0);
    reason
}

fn beneficiary(vout: u32) -> GenesisSeal {
    let txid = Txid::from_str(BENEFICIARY_TXID).unwrap();
    GenesisSeal::with_blinding(txid, vout, 100_000 + u64::from(vout))
}

fn terms() -> ContractTerms {
    ContractTerms {
        text: RicardianContract::default(),
        media: None,
    }
}

fn issuer<C: IssuerWrapper>() -> ContractBuilder {
    ContractBuilder::with(
        "ssi:anonymous".into(),
        C::schema_rules(),
        ChainNet::BitcoinTestnet4,
    )
}

/// rgb-schemas `examples/bfa.rs`, plus the optional `details` it leaves out.
pub fn bfa_contract() -> ValidContract {
    let location = BridgeLocation::Evm(EvmContract {
        chain_id: 1,
        address: EvmAddress::from_str("0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef").unwrap(),
    });
    let reject_list = RejectListLocation::Url(RejectListUrl::from("example.xyz/reject"));
    issuer::<BridgedFungibleAsset>()
        .add_global_state("ticker", UnicodeTicker::from(BFA_TICKER))
        .and_then(|b| b.add_global_state("name", Name::from("Test asset")))
        .and_then(|b| b.add_global_state("details", Details::from(BFA_DETAILS)))
        .and_then(|b| b.add_global_state("precision", Precision::CentiMicro))
        .and_then(|b| b.add_global_state("terms", terms()))
        .and_then(|b| b.add_global_state("rejectList", reject_list))
        .and_then(|b| b.add_global_state("bridgeLocation", location))
        .and_then(|b| b.add_rights("mintRight", beneficiary(1)))
        .and_then(|b| b.add_rights("mintRight", beneficiary(2)))
        .and_then(|b| b.issue_contract_raw(CREATED_AT))
        .expect("BFA contract issues")
}

/// rgb-schemas `examples/ifa.rs`.
pub fn ifa_contract() -> ValidContract {
    let spec = AssetSpec::new("TEST", "Test asset", Precision::CentiMicro);
    issuer::<InflatableFungibleAsset>()
        .add_global_state("spec", spec)
        .and_then(|b| b.add_global_state("terms", terms()))
        .and_then(|b| b.add_global_state("issuedSupply", Amount::from(ISSUED)))
        .and_then(|b| b.add_global_state("maxSupply", Amount::from(MAX_SUPPLY)))
        .and_then(|b| {
            b.add_global_state("rejectListUrl", RejectListUrl::from("example.xyz/reject"))
        })
        .and_then(|b| b.add_fungible_state("assetOwner", beneficiary(1), ISSUED))
        .and_then(|b| {
            b.add_fungible_state("inflationAllowance", beneficiary(2), MAX_SUPPLY - ISSUED)
        })
        .and_then(|b| b.issue_contract_raw(CREATED_AT))
        .expect("IFA contract issues")
}

/// rgb-schemas `examples/cfa.rs`.
pub fn cfa_contract() -> ValidContract {
    issuer::<CollectibleFungibleAsset>()
        .add_global_state("name", Name::from("Test asset"))
        .and_then(|b| b.add_global_state("precision", Precision::CentiMicro))
        .and_then(|b| b.add_global_state("terms", terms()))
        .and_then(|b| b.add_global_state("issuedSupply", Amount::from(ISSUED)))
        .and_then(|b| b.add_fungible_state("assetOwner", beneficiary(1), ISSUED))
        .and_then(|b| b.issue_contract_raw(CREATED_AT))
        .expect("CFA contract issues")
}

/// The pieces a test asserts against, next to the transfer built from them.
pub struct Built {
    pub transfer: Transfer,
    pub propagate: OpId,
    pub mint: OpId,
    pub split: OpId,
    pub burn: OpId,
}

/// A burn of the BFA contract the way rgb-lib f3975623 composes one: the mint
/// right comes from a propagate, and the burn tx holds a split and the burn
/// spending its output.
pub fn build() -> Built {
    let genesis = bfa_contract().into_consignment().genesis;
    let contract_id = genesis.contract_id();

    let propagate = bfa_transition(contract_id, TS_PROPAGATE)
        .add_input(Opout::new(genesis.id(), OS_MINT, 0), AllocatedState::Void)
        .and_then(|b| b.add_rights("mintRight", GraphSeal::with_blinded_vout(0, 1)))
        .and_then(|b| b.add_rights("mintRight", GraphSeal::with_blinded_vout(1, 2)))
        .and_then(|b| b.complete_transition())
        .unwrap();
    let propagate_bundle = witness_bundle(1, vec![], vec![propagate.clone()]);

    // Bridge mints pay a witness invoice: a seal on an output of the mint tx.
    let mint = bfa_transition(contract_id, TS_MINT)
        .add_input(Opout::new(propagate.id(), OS_MINT, 0), AllocatedState::Void)
        .and_then(|b| b.add_global_state("issuedSupply", Amount::from(MINTED)))
        .and_then(|b| b.add_metadata("beforeBlock", BlockNumber::from(BEFORE_BLOCK)))
        .and_then(|b| {
            b.add_fungible_state("assetOwner", GraphSeal::with_blinded_vout(0, 3), MINTED)
        })
        .and_then(|b| b.complete_transition())
        .unwrap();
    let mint_bundle = witness_bundle(2, vec![], vec![mint.clone()]);
    let minted_outpoint = OutPoint::new(mint_bundle.witness_id(), 0);

    // rgb-lib puts the split output on an outpoint the burn tx spends.
    let split_seal = GraphSeal::with_blinding(minted_outpoint.txid, 0, 5);
    let split = bfa_transition(contract_id, TS_TRANSFER)
        .add_input(
            Opout::new(mint.id(), OS_ASSET, 0),
            Amount::from(MINTED).into(),
        )
        .and_then(|b| {
            b.add_fungible_state(
                "assetOwner",
                GraphSeal::with_blinded_vout(1, 7),
                MINTED - BURNED,
            )
        })
        .and_then(|b| b.add_fungible_state("assetOwner", split_seal, BURNED))
        .and_then(|b| b.complete_transition())
        .unwrap();
    let burn = bfa_transition(contract_id, TS_BURN)
        .add_input(split_opout(&split, split_seal), Amount::from(BURNED).into())
        .and_then(|b| b.add_global_state("burnedAsset", Amount::from(BURNED)))
        .and_then(|b| {
            b.add_global_state(
                "burnReason",
                BurnReason::from(Bytes32::from_byte_array(burn_reason())),
            )
        })
        .and_then(|b| b.complete_transition())
        .unwrap();
    let burn_bundle = witness_bundle(3, vec![minted_outpoint], vec![split.clone(), burn.clone()]);

    let bundles = vec![propagate_bundle, mint_bundle, burn_bundle];
    let transfer = Transfer {
        version: ConsignmentVer::V1,
        transfer: true,
        genesis,
        bundles: LargeVec::try_from(bundles).unwrap(),
    };
    Built {
        transfer,
        propagate: propagate.id(),
        mint: mint.id(),
        split: split.id(),
        burn: burn.id(),
    }
}

/// The container written as rgb-lib writes `consignment_out` (`RGB\0TFR` for a transfer).
pub fn file_bytes(container: &impl FileContent) -> Vec<u8> {
    let mut bytes = Vec::new();
    container.save(&mut bytes).unwrap();
    bytes
}

/// The container as rgb-ops `ValidConsignment::save_file` writes it: `RGB\0VTF`
/// or `RGB\0VCO`, the same body, then the validation status as JSON.
pub fn validated_file_bytes(container: &impl FileContent, status: &Status) -> Vec<u8> {
    let mut bytes = file_bytes(container);
    let magic = if &bytes[4..7] == b"TFR" {
        b"VTF"
    } else {
        b"VCO"
    };
    bytes[4..7].copy_from_slice(magic);
    serde_json::to_writer(&mut bytes, status).unwrap();
    bytes
}

/// A contract as rgb-lib keeps an issued asset file.
pub fn contract_file_bytes(contract: &ValidContract) -> Vec<u8> {
    validated_file_bytes(&**contract, contract.validation_status())
}

fn bfa_transition(contract_id: ContractId, transition_type: TransitionType) -> TransitionBuilder {
    let (schema, types, _) = BridgedFungibleAsset::schema_rules().into_parts();
    TransitionBuilder::with(contract_id, schema, transition_type, types)
}

// The builder sorts assignments by seal, so the split output's index is looked up.
fn split_opout(split: &Transition, seal: GraphSeal) -> Opout {
    let assigns = split.assignments.get(&OS_ASSET).unwrap().as_fungible();
    let no = assigns
        .iter()
        .position(|a| a.revealed_seal() == Some(seal))
        .unwrap();
    Opout::new(split.id(), OS_ASSET, no as u16)
}

// Transitions go in spending order, as rgb-api sorts a bundle.
fn witness_bundle(
    lock_time: u32,
    spends: Vec<OutPoint>,
    transitions: Vec<Transition>,
) -> WitnessBundle {
    let input_map: BTreeMap<Opout, OpId> = transitions
        .iter()
        .flat_map(|t| (&t.inputs).into_iter().map(move |opout| (opout, t.id())))
        .collect();
    let known = transitions
        .into_iter()
        .map(|t| KnownTransition::new(t.id(), t))
        .collect::<Vec<_>>();
    let mut wb = WitnessBundle::strict_dumb();
    wb.tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::from_consensus(lock_time),
        input: spends
            .into_iter()
            .map(|previous_output| TxIn {
                previous_output,
                ..TxIn::default()
            })
            .collect(),
        output: vec![],
    };
    wb.bundle = TransitionBundle {
        input_map: NonEmptyOrdMap::try_from(input_map).unwrap(),
        known_transitions: NonEmptyVec::try_from(known).unwrap(),
    };
    wb
}
