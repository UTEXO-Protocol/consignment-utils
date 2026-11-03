package rgbconsignment

import (
	"bytes"
	"encoding/binary"
	"encoding/hex"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// tests/synthetic in the Rust crate builds the fixtures with the rgb-lib 1ae1c160 revs: the
// rgb-schemas example contracts, and a transfer valid against its own witness txs.
const (
	bfaFixture              = "../testdata/bfa-contract.rgb"
	ifaFixture              = "../testdata/ifa-contract.rgb"
	transferFixture         = "../testdata/bfa-transfer-synthetic.rgb"
	legacyV0Fixture         = "../testdata/legacy-v0-bfa-burn.rgb"
	legacyTerminalsContract = "../testdata/legacy-terminals-bfa-contract.rgb"
	legacyTerminalsTransfer = "../testdata/legacy-terminals-bfa-transfer.rgb"
)

// BFA ids from rgb-schemas 641a06f (src/lib.rs).
const (
	tsBurn         uint16 = 8010
	tsMint         uint16 = 8013
	tsPropagate    uint16 = 8014
	tsTransfer     uint16 = 10000
	osAsset        uint16 = 4000
	gsIssuedSupply uint16 = 2010
	gsBurnedAsset  uint16 = 2015
	gsBridgeLoc    uint16 = 3008
	gsBurnReason   uint16 = 3009
	msBeforeBlock  uint16 = 1003
)

func parseFixture(t testing.TB, path string) *Info {
	t.Helper()
	info, err := ParseFile(filepath.FromSlash(path))
	if err != nil {
		t.Fatalf("ParseFile(%s): %v", path, err)
	}
	return info
}

func TestParseFileBFAContract(t *testing.T) {
	info := parseFixture(t, bfaFixture)
	if !info.IsContract() || info.Version != 1 {
		t.Fatalf("kind/version = %q/%d, want contract/1", info.Kind, info.Version)
	}
	g := info.Genesis
	strEq(t, "contract_id", g.ContractID, "rgb:IP7rzsJ3-KTivsnR-lwYBS0E-dIKpPU8-OayQsVt-JI~hdCA")
	strEq(t, "schema_id", g.SchemaID, "rgb:sch:HbLJ6SjvzVw~XTu6e~Iau6wIcxYsVTlSM7dRUn0DDo4#brave-type-culture")
	strEq(t, "top-level schema_id", info.SchemaID, g.SchemaID)
	strEq(t, "chain_net", g.ChainNet, "tb4")
	// BFA keeps its nominals as root-level globals; the ticker may be any Unicode.
	strPtrEq(t, "name", g.Name, "Test asset")
	strPtrEq(t, "ticker", g.Ticker, "₮EST")
	strPtrEq(t, "details", g.Details, "Bridged test asset")
	if g.Precision == nil || *g.Precision != 8 {
		t.Errorf("precision = %v", g.Precision)
	}
	if len(g.FungibleAllocations) != 0 || len(info.Witnesses) != 0 {
		t.Errorf("BFA genesis must allocate no asset and a contract has no history: %+v", info)
	}
}

func TestBFAGenesisBridgeLocation(t *testing.T) {
	g := parseFixture(t, bfaFixture).Genesis
	location, err := g.GlobalOnce(gsBridgeLoc)
	if err != nil {
		t.Fatal(err)
	}
	// BridgeLocation::Evm: union tag 0, chain id (u64 LE), 20-byte address.
	if len(location) != 29 || location[0] != 0 {
		t.Fatalf("bridgeLocation = %x", location)
	}
	if chainID := binary.LittleEndian.Uint64(location[1:9]); chainID != 1 {
		t.Errorf("chain id = %d, want 1", chainID)
	}
	strEq(t, "address", hex.EncodeToString(location[9:]), strings.Repeat("deadbeef", 5))
}

func TestParseFileIFAContractAllocations(t *testing.T) {
	g := parseFixture(t, ifaFixture).Genesis
	if len(g.FungibleAllocations) != 2 {
		t.Fatalf("fungible_allocations len = %d, want 2", len(g.FungibleAllocations))
	}
	a := g.FungibleAllocations[0]
	if a.AssignmentType != osAsset || a.Total != 100000 || len(a.Entries) != 1 {
		t.Fatalf("alloc[0] = %+v", a)
	}
	e := a.Entries[0]
	if e.Seal.Kind != SealRevealed || e.Seal.Vout != 1 || e.Seal.Txid == nil ||
		*e.Seal.Txid != "14295d5bb1a191cdb6286dc0944df938421e3dfcbf0811353ccac4100c2068c5" {
		t.Errorf("entry[0] = %+v", e)
	}
}

func TestParseSyntheticTransferPropagate(t *testing.T) {
	info := parseFixture(t, transferFixture)
	if !info.IsTransfer() || len(info.Witnesses) != 3 || info.BundleCount != 3 {
		t.Fatalf("kind=%q witnesses=%d bundles=%d", info.Kind, len(info.Witnesses), info.BundleCount)
	}
	propagate := info.Witnesses[0].Transitions[0]
	if propagate.TransitionType != tsPropagate || len(propagate.FungibleAllocations) != 0 {
		t.Errorf("propagate = %+v, want TS_PROPAGATE moving only declarative rights", propagate)
	}
}

func TestParseSyntheticTransferMint(t *testing.T) {
	mint := parseFixture(t, transferFixture).Witnesses[1].Transitions[0]
	if mint.TransitionType != tsMint {
		t.Fatalf("transition type = %d, want TS_MINT", mint.TransitionType)
	}
	expectU64(t, "issuedSupply", mint.Global, gsIssuedSupply, 1000)
	expectU64(t, "beforeBlock", mint.Meta, msBeforeBlock, 42)
	if len(mint.FungibleAllocations) != 1 || mint.FungibleAllocations[0].Total != 1000 {
		t.Errorf("mint allocations = %+v", mint.FungibleAllocations)
	}
}

func TestParseSyntheticTransferSplitThenBurn(t *testing.T) {
	info := parseFixture(t, transferFixture)
	last := info.Witnesses[2]
	if len(last.Transitions) != 2 || last.Transitions[0].TransitionType != tsTransfer ||
		last.Transitions[1].TransitionType != tsBurn {
		t.Fatalf("burn witness transitions = %+v, want TS_TRANSFER then TS_BURN", last.Transitions)
	}
	split, burn := last.Transitions[0], last.Transitions[1]

	expectU64(t, "burnedAsset", burn.Global, gsBurnedAsset, 400)
	reason, ok := burn.Global(gsBurnReason)
	want := append(make([]byte, 12), bytes.Repeat([]byte{0xab}, 20)...)
	if !ok || !bytes.Equal(reason, want) {
		t.Errorf("burnReason = %x, ok=%v, want %x", reason, ok, want)
	}
	if len(burn.FungibleAllocations) != 0 || burn.InputCount != 1 {
		t.Errorf("burn must spend one output and assign nothing: %+v", burn)
	}

	// The split pays the change and puts the burned part on the mint output the burn tx spends.
	entries := split.FungibleAllocations[0].Entries
	if len(entries) != 2 {
		t.Fatalf("split entries = %+v", entries)
	}
	change, burned := entries[0], entries[1]
	if change.Amount != 600 || change.Seal.Kind != SealRevealed || change.Seal.Txid != nil || change.Seal.Vout != 1 {
		t.Errorf("change = %+v", change)
	}
	if burned.Amount != 400 || burned.Seal.Txid == nil || *burned.Seal.Txid != info.Witnesses[1].Txid {
		t.Errorf("burned output = %+v, want it on the mint witness %s", burned, info.Witnesses[1].Txid)
	}
}

// The bridge settles the one burn of the last witness and cites the mints of the history.
func TestBurnConsignmentEndsInOneBurnAfterItsMint(t *testing.T) {
	info := parseFixture(t, transferFixture)
	last := len(info.Witnesses) - 1
	var burnsInLast, mintsBefore int
	for i, w := range info.Witnesses {
		for _, tr := range w.Transitions {
			if tr.TransitionType == tsBurn && i == last {
				burnsInLast++
			}
			if tr.TransitionType == tsMint && i < last {
				mintsBefore++
			}
		}
	}
	if burnsInLast != 1 || mintsBefore != 1 {
		t.Errorf("burns in the last witness = %d, mints before it = %d, want 1 and 1", burnsInLast, mintsBefore)
	}
}

func TestParseRejectsLegacyV0Transfer(t *testing.T) {
	_, err := ParseFile(filepath.FromSlash(legacyV0Fixture))
	if err == nil || !strings.Contains(err.Error(), "ConsignmentVer") {
		t.Fatalf("legacy V0 transfer must be rejected on its version, got err=%v", err)
	}
}

func TestParseRejectsTerminalsLayout(t *testing.T) {
	for _, path := range []string{legacyTerminalsContract, legacyTerminalsTransfer} {
		_, err := ParseFile(filepath.FromSlash(path))
		if err == nil || !strings.Contains(err.Error(), "V1 with history terminals") {
			t.Errorf("%s: a pre-f3975623 V1 file must be rejected as such, got err=%v", path, err)
		}
	}
}

func expectU64(t *testing.T, what string, lookup func(uint16) ([]byte, bool), id uint16, want uint64) {
	t.Helper()
	raw, ok := lookup(id)
	if !ok || len(raw) != 8 {
		t.Errorf("%s = %x, ok=%v, want 8 bytes", what, raw, ok)
		return
	}
	if got := binary.LittleEndian.Uint64(raw); got != want {
		t.Errorf("%s = %d, want %d", what, got, want)
	}
}

func TestParseRejectsTruncatedFixture(t *testing.T) {
	data, err := os.ReadFile(filepath.FromSlash(transferFixture))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := Parse(data[:len(data)/2]); err == nil {
		t.Fatal("expected error for truncated consignment")
	}
}

func BenchmarkParseSyntheticTransfer(b *testing.B) {
	data, err := os.ReadFile(filepath.FromSlash(transferFixture))
	if err != nil {
		b.Fatal(err)
	}
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		if _, err := Parse(data); err != nil {
			b.Fatal(err)
		}
	}
}

func strEq(t *testing.T, what, got, want string) {
	t.Helper()
	if got != want {
		t.Errorf("%s = %q, want %q", what, got, want)
	}
}

func strPtrEq(t *testing.T, what string, got *string, want string) {
	t.Helper()
	if got == nil {
		t.Errorf("%s = nil, want %q", what, want)
		return
	}
	strEq(t, what, *got, want)
}
