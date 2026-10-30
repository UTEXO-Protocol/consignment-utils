//! Builds the fixtures with the rgb-lib 1ae1c160 revs: the rgb-schemas example contracts,
//! and a BFA transfer committing each bundle in an OP_RETURN, so it passes [`validate`].

use std::collections::BTreeMap;
use std::str::FromStr;

use amplify::Bytes32;
use amplify::confinement::{LargeVec, NonEmptyOrdMap, NonEmptyVec};
use rgbstd::bitcoin::absolute::LockTime;
use rgbstd::bitcoin::hashes::Hash;
use rgbstd::bitcoin::opcodes::all::OP_RETURN;
use rgbstd::bitcoin::script::Builder;
use rgbstd::bitcoin::transaction::Version;
use rgbstd::bitcoin::{Amount as Sats, OutPoint, ScriptBuf, Transaction, TxIn, TxOut, WPubkeyHash};
use rgbstd::containers::{
    AnchorResolverError, ConsignmentVer, FileContent, ResolveAnchor, Transfer, ValidContract,
    WitnessBundle,
};
use rgbstd::contract::{AllocatedState, ContractBuilder, IssuerWrapper, TransitionBuilder};
use rgbstd::opret::{OpretFirst, OpretProof};
use rgbstd::rgbcore::commit_verify::mpc::{
    Commitment, MerkleBlock, MerkleTree, Message, MultiSource, ProtocolId,
};
use rgbstd::rgbcore::commit_verify::{CommitId, EmbedCommitVerify, TryCommitVerify};
use rgbstd::stl::{
    AssetSpec, BlockNumber, BridgeLocation, BurnReason, ContractTerms, Details, EvmAddress,
    EvmContract, Name, RejectListLocation, RejectListUrl, RicardianContract, UnicodeTicker,
};
use rgbstd::validation::{
    DbcProof, ExternalAnchor, ResolveWitness, Status, ValidationConfig, WitnessResolverError,
    WitnessStatus,
};
use rgbstd::vm::WitnessOrd;
use rgbstd::{
    Amount, Anchor, AssignmentType, ChainNet, ContractId, Genesis, GenesisSeal, GraphSeal,
    KnownTransition, OpId, Operation, Opout, Precision, Transition, TransitionBundle,
    TransitionType, Txid,
};
use schemata::{
    BridgedFungibleAsset, CollectibleFungibleAsset, InflatableFungibleAsset, OS_ASSET, OS_MINT,
    TS_BURN, TS_MINT, TS_PROPAGATE, TS_TRANSFER,
};

/// rgb-schemas `examples/common`: the issuance time and the beneficiary tx.
const CREATED_AT: i64 = 1713261744;
const BENEFICIARY_TXID: &str = "14295d5bb1a191cdb6286dc0944df938421e3dfcbf0811353ccac4100c2068c5";
/// The chain id of the BFA bridge location.
const BRIDGE_CHAIN_ID: u64 = 1;

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
        chain_id: BRIDGE_CHAIN_ID,
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

/// A BFA burn as rgb-lib 1ae1c160 composes one: the mint spends a right a propagate
/// made, and the burn tx holds a split and the burn spending its output.
pub fn build() -> Built {
    let genesis = bfa_contract().into_consignment().genesis;
    let mut chain = Witnesses::new(genesis.contract_id());

    let genesis_right = Opout::new(genesis.id(), OS_MINT, 0);
    let (propagate, propagate_txid) =
        chain.propagate(genesis_right, seal_outpoint(&genesis, genesis_right));
    let right = opout_on(&propagate, OS_MINT, right_seal());
    let (mint, mint_txid) = chain.mint(right, OutPoint::new(propagate_txid, 1));
    let minted = OutPoint::new(mint_txid, 1);
    let (split, burn) = chain.burn(Opout::new(mint.id(), OS_ASSET, 0), minted);

    let transfer = Transfer {
        version: ConsignmentVer::V1,
        transfer: true,
        genesis,
        bundles: LargeVec::try_from(chain.bundles).unwrap(),
    };
    Built {
        transfer,
        propagate: propagate.id(),
        mint: mint.id(),
        split: split.id(),
        burn: burn.id(),
    }
}

/// The right a rights send puts on vout 1 of its tx.
fn right_seal() -> GraphSeal {
    GraphSeal::with_blinded_vout(1, 1)
}

/// The witness txs of a contract, each committing one bundle.
struct Witnesses {
    contract_id: ContractId,
    bundles: Vec<WitnessBundle>,
}

impl Witnesses {
    fn new(contract_id: ContractId) -> Self {
        Self {
            contract_id,
            bundles: Vec::new(),
        }
    }

    /// A rights send: moves `right` (on `outpoint`) to rights on vouts 1 and 2.
    fn propagate(&mut self, right: Opout, outpoint: OutPoint) -> (Transition, Txid) {
        let propagate = bfa_transition(self.contract_id, TS_PROPAGATE)
            .add_input(right, AllocatedState::Void)
            .and_then(|b| b.add_rights("mintRight", right_seal()))
            .and_then(|b| b.add_rights("mintRight", GraphSeal::with_blinded_vout(2, 2)))
            .and_then(|b| b.complete_transition())
            .unwrap();
        let txid = self.commit(vec![outpoint], 2, vec![propagate.clone()]);
        (propagate, txid)
    }

    /// A bridge mint spending `right`, paying a witness invoice on vout 1 of its tx.
    fn mint(&mut self, right: Opout, outpoint: OutPoint) -> (Transition, Txid) {
        let mint = bfa_transition(self.contract_id, TS_MINT)
            .add_input(right, AllocatedState::Void)
            .and_then(|b| b.add_global_state("issuedSupply", Amount::from(MINTED)))
            .and_then(|b| b.add_metadata("beforeBlock", BlockNumber::from(BEFORE_BLOCK)))
            .and_then(|b| {
                b.add_fungible_state("assetOwner", GraphSeal::with_blinded_vout(1, 3), MINTED)
            })
            .and_then(|b| b.complete_transition())
            .unwrap();
        let txid = self.commit(vec![outpoint], 1, vec![mint.clone()]);
        (mint, txid)
    }

    /// Burns part of the `MINTED` on `outpoint`: the split pays the change to vout 1
    /// and the burned part to `outpoint`, as rgb-lib does, then the burn spends it.
    fn burn(&mut self, input: Opout, outpoint: OutPoint) -> (Transition, Transition) {
        let split_seal = GraphSeal::with_blinding(outpoint.txid, outpoint.vout, 5);
        let change = GraphSeal::with_blinded_vout(1, 7);
        let split = bfa_transition(self.contract_id, TS_TRANSFER)
            .add_input(input, Amount::from(MINTED).into())
            .and_then(|b| b.add_fungible_state("assetOwner", change, MINTED - BURNED))
            .and_then(|b| b.add_fungible_state("assetOwner", split_seal, BURNED))
            .and_then(|b| b.complete_transition())
            .unwrap();
        let reason = BurnReason::from(Bytes32::from_byte_array(burn_reason()));
        let burn = bfa_transition(self.contract_id, TS_BURN)
            .add_input(
                opout_on(&split, OS_ASSET, split_seal),
                Amount::from(BURNED).into(),
            )
            .and_then(|b| b.add_global_state("burnedAsset", Amount::from(BURNED)))
            .and_then(|b| b.add_global_state("burnReason", reason))
            .and_then(|b| b.complete_transition())
            .unwrap();
        self.commit(vec![outpoint], 1, vec![split.clone(), burn.clone()]);
        (split, burn)
    }

    /// Commits `transitions` as one bundle in a tx spending `spends`, with the
    /// OP_RETURN commitment on vout 0 and `outputs` dust outputs after it.
    fn commit(&mut self, spends: Vec<OutPoint>, outputs: u8, transitions: Vec<Transition>) -> Txid {
        let bundle = bundle_of(transitions);
        let tree = self.mpc_tree(&bundle);
        let mut tx = witness_tx(spends, outputs);
        <Transaction as EmbedCommitVerify<Commitment, OpretFirst>>::embed_commit(
            &mut tx,
            &tree.commit_id(),
        )
        .unwrap();
        let protocol = ProtocolId::from(self.contract_id);
        let mpc_proof = MerkleBlock::from(tree).to_merkle_proof(protocol).unwrap();
        let txid = tx.compute_txid();
        self.bundles.push(WitnessBundle {
            tx,
            spv_proof: None,
            anchor: Anchor::new(mpc_proof, DbcProof::Opret(OpretProof::default())),
            bundle,
        });
        txid
    }

    /// The MPC tree of the next bundle: its one message, with fixed entropy.
    fn mpc_tree(&self, bundle: &TransitionBundle) -> MerkleTree {
        let mut source = MultiSource {
            static_entropy: Some(0x5eed + self.bundles.len() as u64),
            ..Default::default()
        };
        source
            .messages
            .insert(
                ProtocolId::from(self.contract_id),
                Message::from(bundle.bundle_id()),
            )
            .unwrap();
        MerkleTree::try_commit(&source).unwrap()
    }
}

/// The outpoint of the genesis seal `opout` assigns to.
fn seal_outpoint(genesis: &Genesis, opout: Opout) -> OutPoint {
    let assigns = genesis.assignments.get(&opout.ty).unwrap();
    let seal = assigns.revealed_seal_at(opout.no).unwrap().unwrap();
    OutPoint::new(seal.txid, seal.vout.to_u32())
}

// The builder sorts assignments by seal, so an output's index is looked up.
fn opout_on(transition: &Transition, ty: AssignmentType, seal: GraphSeal) -> Opout {
    let assigns = transition.assignments.get(&ty).unwrap();
    let no = assigns
        .seals()
        .position(|s| s.revealed() == Some(seal))
        .unwrap();
    Opout::new(transition.id(), ty, no as u16)
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

/// Validates `transfer` with the pinned rgb-ops as rgb-lib does for BFA (with
/// the ops DAG): its own txs are the witnesses and its one mint is locked.
pub fn validate(transfer: &Transfer, mint: OpId) -> Result<Status, String> {
    let config = ValidationConfig {
        chain_net: ChainNet::BitcoinTestnet4,
        safe_height: None,
        build_ops_dag: true,
    };
    let rules = BridgedFungibleAsset::schema_rules();
    let mut pending = transfer
        .clone()
        .validate_deterministic(&rules, &config)
        .map_err(|e| format!("phase 1: {e}"))?;
    let location = BridgedFungibleAsset::bridge_location(&transfer.genesis).unwrap();
    pending
        .resolve_all_anchors(location, &LockedMint(mint))
        .map_err(|e| format!("mint anchor: {e}"))?;
    let txs = transfer.bundles.iter().map(|wb| wb.tx.clone()).collect();
    pending
        .pending()
        .resolve_all(&OwnWitnesses(txs))
        .map_err(|e| format!("witnesses: {e}"))?;
    Ok(pending.finalize().into_validation_status())
}

/// Resolves a witness to the tx the transfer carries, unmined.
struct OwnWitnesses(Vec<Transaction>);

impl ResolveWitness for OwnWitnesses {
    fn resolve_witness(&self, witness_id: Txid) -> Result<WitnessStatus, WitnessResolverError> {
        let tx = self.0.iter().find(|tx| tx.compute_txid() == witness_id);
        Ok(tx.map_or(WitnessStatus::Unresolved, |tx| {
            WitnessStatus::Resolved(tx.clone(), WitnessOrd::Tentative)
        }))
    }

    fn check_chain_net(&self, _: ChainNet) -> Result<(), WitnessResolverError> {
        Ok(())
    }
}

/// The bridge's view: only the given mint of `MINTED` before `BEFORE_BLOCK` is locked.
struct LockedMint(OpId);

impl ResolveAnchor for LockedMint {
    fn evm_chain_id(&self) -> Result<u64, AnchorResolverError> {
        Ok(BRIDGE_CHAIN_ID)
    }

    fn is_confirmed(
        &self,
        _: &BridgeLocation,
        anchor: &ExternalAnchor,
    ) -> Result<bool, AnchorResolverError> {
        let locked = ExternalAnchor::Mint {
            opid: self.0,
            amount: MINTED,
            before_block: BEFORE_BLOCK,
        };
        Ok(*anchor == locked)
    }
}

fn bfa_transition(contract_id: ContractId, transition_type: TransitionType) -> TransitionBuilder {
    let (schema, types, _) = BridgedFungibleAsset::schema_rules().into_parts();
    TransitionBuilder::with(contract_id, schema, transition_type, types)
}

/// One bundle of `transitions`, in spending order as rgb-api sorts a bundle.
fn bundle_of(transitions: Vec<Transition>) -> TransitionBundle {
    let input_map: BTreeMap<Opout, OpId> = transitions
        .iter()
        .flat_map(|t| (&t.inputs).into_iter().map(move |opout| (opout, t.id())))
        .collect();
    let known = transitions
        .into_iter()
        .map(|t| KnownTransition::new(t.id(), t))
        .collect::<Vec<_>>();
    TransitionBundle {
        input_map: NonEmptyOrdMap::try_from(input_map).unwrap(),
        known_transitions: NonEmptyVec::try_from(known).unwrap(),
    }
}

/// A tx spending `spends`, with an empty OP_RETURN on vout 0 and `outputs` dust outputs.
fn witness_tx(spends: Vec<OutPoint>, outputs: u8) -> Transaction {
    let commitment = TxOut {
        value: Sats::ZERO,
        script_pubkey: Builder::new().push_opcode(OP_RETURN).into_script(),
    };
    let dust = (1..=outputs).map(|tag| TxOut {
        value: Sats::from_sat(1_000),
        script_pubkey: ScriptBuf::new_p2wpkh(&WPubkeyHash::from_byte_array([tag; 20])),
    });
    Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: spends
            .into_iter()
            .map(|previous_output| TxIn {
                previous_output,
                ..TxIn::default()
            })
            .collect(),
        output: std::iter::once(commitment).chain(dust).collect(),
    }
}
