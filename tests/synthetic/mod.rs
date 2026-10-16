//! Builds a BFA mint + burn transfer with the RGB 0.11.2-rc.1 types.
//!
//! Stand-in until release-produced transfers exist: the operations are well
//! formed but the anchors are dummies, so it would never validate.

use std::collections::BTreeMap;

use amplify::Bytes32;
use amplify::confinement::{
    Confined, LargeVec, NonEmptyOrdMap, NonEmptyOrdSet, NonEmptyVec, SmallBlob, SmallOrdMap, U16,
    U32,
};
use rgbstd::assignments::AssignVec;
use rgbstd::bitcoin::Transaction;
use rgbstd::bitcoin::absolute::LockTime;
use rgbstd::bitcoin::transaction::Version;
use rgbstd::containers::{
    Consignment, ConsignmentVer, FileContent, TerminalSeals, Transfer, WitnessBundle,
};
use rgbstd::rgbcore::validation::Status;
use rgbstd::stl::{BlockNumber, BurnReason};
use rgbstd::{
    Amount, Assign, AssignmentType, Assignments, BuilderSeal, Genesis, GlobalState,
    GlobalStateType, GraphSeal, Inputs, KnownTransition, MetaType, MetaValue, Metadata, OpId,
    Operation, Opout, RevealedData, RevealedValue, Transition, TransitionBundle, TransitionType,
    TypedAssigns,
};
use strict_types::encoding::{StreamReader, StrictDecode, StrictDumb, StrictSerialize};

pub const TS_MINT: u16 = 8013;
pub const TS_BURN: u16 = 8010;
pub const OS_ASSET: u16 = 4000;
pub const OS_MINT: u16 = 4013;
pub const GS_ISSUED_SUPPLY: u16 = 2010;
pub const GS_BURNED_ASSET: u16 = 2015;
pub const GS_BURN_REASON: u16 = 3009;
pub const MS_AFTER_BLOCK: u16 = 1003;

pub const MINTED: u64 = 1000;
pub const BURNED: u64 = 400;
pub const AFTER_BLOCK: u64 = 42;

/// 12 zero bytes + a 20-byte EVM recipient, the bridge's burn reason layout.
pub fn burn_reason() -> [u8; 32] {
    let mut reason = [0xab; 32];
    reason[..12].fill(0);
    reason
}

/// The pieces a test asserts against, next to the transfer built from them.
pub struct Built {
    pub transfer: Transfer,
    pub mint: OpId,
    pub burn: OpId,
    pub change_seal: GraphSeal,
}

/// Genesis of the rgb-schemas BFA example contract (a `VCO` container).
pub fn bfa_genesis(contract_file: &[u8]) -> Genesis {
    let body = &contract_file[7..];
    Consignment::<false>::strict_read(StreamReader::new::<U32>(body))
        .expect("BFA example contract decodes")
        .genesis
}

pub fn build(genesis: Genesis) -> Built {
    let change_seal = GraphSeal::with_blinded_vout(1, 7);
    let terminal_seal = GraphSeal::with_blinded_vout(2, 9);

    let mint = mint_transition(&genesis);
    let mint_id = mint.id();
    let burn = burn_transition(&genesis, mint_id, change_seal);
    let burn_id = burn.id();

    let mint_bundle = witness_bundle(1, mint);
    let burn_bundle = witness_bundle(2, burn);
    let seals = vec![
        BuilderSeal::Concealed(change_seal.to_secret_seal()),
        BuilderSeal::Revealed(terminal_seal),
    ];
    let terminals = BTreeMap::from([(
        burn_bundle.bundle.bundle_id(),
        TerminalSeals::from(NonEmptyVec::try_from(seals).unwrap()),
    )]);

    let transfer = Transfer {
        version: ConsignmentVer::V1,
        transfer: true,
        terminals: SmallOrdMap::try_from(terminals).unwrap(),
        genesis,
        bundles: LargeVec::try_from(vec![mint_bundle, burn_bundle]).unwrap(),
    };
    Built {
        transfer,
        mint: mint_id,
        burn: burn_id,
        change_seal,
    }
}

/// The container written as rgb-lib writes `consignment_out` (`RGB\0TFR` for a transfer).
pub fn file_bytes(container: &impl FileContent) -> Vec<u8> {
    let mut bytes = Vec::new();
    container.save(&mut bytes).unwrap();
    bytes
}

/// The transfer written as rgb-lib keeps a validated copy: `RGB\0VTF`, the same
/// body, then the validation status as JSON (rgb-ops `ValidConsignment::save_file`).
pub fn validated_file_bytes(transfer: &Transfer) -> Vec<u8> {
    let mut bytes = file_bytes(transfer);
    bytes[4..7].copy_from_slice(b"VTF");
    serde_json::to_writer(&mut bytes, &Status::default()).unwrap();
    bytes
}

fn mint_transition(genesis: &Genesis) -> Transition {
    let mut metadata = Metadata::default();
    metadata
        .add_value(
            MetaType::with(MS_AFTER_BLOCK),
            MetaValue::from(blob(&BlockNumber::from(AFTER_BLOCK))),
        )
        .unwrap();
    let mut globals = GlobalState::default();
    add_global(&mut globals, GS_ISSUED_SUPPLY, &Amount::from(MINTED));
    // A witness-vout seal: no txid, it points into the witness tx.
    let seal = BuilderSeal::Revealed(GraphSeal::with_blinded_vout(0, 3));
    transition(
        genesis,
        TS_MINT,
        metadata,
        globals,
        Opout::new(genesis.id(), AssignmentType::with(OS_MINT), 0),
        seal,
        MINTED,
    )
}

fn burn_transition(genesis: &Genesis, mint: OpId, change: GraphSeal) -> Transition {
    let mut globals = GlobalState::default();
    add_global(&mut globals, GS_BURNED_ASSET, &Amount::from(BURNED));
    add_global(
        &mut globals,
        GS_BURN_REASON,
        &BurnReason::from(Bytes32::from_byte_array(burn_reason())),
    );
    let seal = BuilderSeal::Concealed(change.to_secret_seal());
    let input = Opout::new(mint, AssignmentType::with(OS_ASSET), 0);
    transition(
        genesis,
        TS_BURN,
        Metadata::default(),
        globals,
        input,
        seal,
        MINTED - BURNED,
    )
}

fn transition(
    genesis: &Genesis,
    transition_type: u16,
    metadata: Metadata,
    globals: GlobalState,
    input: Opout,
    seal: BuilderSeal<GraphSeal>,
    amount: u64,
) -> Transition {
    let asset = Assign::with(seal, RevealedValue::from(amount));
    let typed = TypedAssigns::Fungible(AssignVec::with(NonEmptyVec::with(asset)));
    Transition {
        ffv: Default::default(),
        contract_id: genesis.contract_id(),
        nonce: u64::MAX,
        transition_type: TransitionType::with(transition_type),
        metadata,
        globals,
        inputs: Inputs::from(NonEmptyOrdSet::with(input)),
        assignments: Assignments::from(
            Confined::try_from(BTreeMap::from([(AssignmentType::with(OS_ASSET), typed)])).unwrap(),
        ),
        signature: None,
    }
}

fn witness_bundle(lock_time: u32, transition: Transition) -> WitnessBundle {
    let opid = transition.id();
    let input = (&transition.inputs).into_iter().next().unwrap();
    let mut wb = WitnessBundle::strict_dumb();
    wb.tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::from_consensus(lock_time),
        input: vec![],
        output: vec![],
    };
    wb.bundle = TransitionBundle {
        input_map: NonEmptyOrdMap::with_key_value(input, opid),
        known_transitions: NonEmptyVec::with(KnownTransition::new(opid, transition)),
    };
    wb
}

fn add_global(globals: &mut GlobalState, global_type: u16, value: &impl StrictSerialize) {
    globals
        .add_state(
            GlobalStateType::with(global_type),
            RevealedData::new(blob(value)),
        )
        .unwrap();
}

fn blob(value: &impl StrictSerialize) -> SmallBlob {
    value.to_strict_serialized::<U16>().unwrap()
}
