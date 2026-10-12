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

// The contracts are the rgb-schemas 0.11.2-rc.1 examples. The transfer is
// synthetic (built by tests/synthetic in the Rust crate): well formed V1, but
// not produced by a release wallet and not valid against the chain.
const (
	bfaFixture      = "../testdata/bfa-contract.rgb"
	ifaFixture      = "../testdata/ifa-contract.rgb"
	transferFixture = "../testdata/bfa-transfer-synthetic.rgb"
	legacyV0Fixture = "../testdata/legacy-v0-bfa-burn.rgb"
)

// BFA ids from rgb-schemas 0.11.2-rc.1 (src/lib.rs).
const (
	tsMint         uint16 = 8013
	tsBurn         uint16 = 8010
	osAsset        uint16 = 4000
	gsIssuedSupply uint16 = 2010
	gsBurnedAsset  uint16 = 2015
	gsBridgeLoc    uint16 = 3008
	gsBurnReason   uint16 = 3009
	msAfterBlock   uint16 = 1003
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
	strEq(t, "contract_id", g.ContractID, "rgb:UiDQRpwu-~dsy7ZH-raGXsnJ-UNiT13f-WbArf5m-jPeb8xU")
	strEq(t, "schema_id", g.SchemaID, "rgb:sch:Ryl5eQZ3IHrU05e3AWdQZJR4FB5GvxZPy96cwhJiApc#amazon-norway-harris")
	strEq(t, "top-level schema_id", info.SchemaID, g.SchemaID)
	strEq(t, "chain_net", g.ChainNet, "tb4")
	if g.Name == nil || *g.Name != "Test asset" || g.Ticker == nil || *g.Ticker != "TEST" {
		t.Errorf("name/ticker = %v/%v", g.Name, g.Ticker)
	}
	if g.Precision == nil || *g.Precision != 8 {
		t.Errorf("precision = %v", g.Precision)
	}
	if len(g.FungibleAllocations) != 0 || len(info.Witnesses) != 0 || len(info.Terminals) != 0 {
		t.Errorf("BFA genesis must allocate no asset and a contract has no history: %+v", info)
	}
}

func TestBFAGenesisBridgeLocation(t *testing.T) {
	g := parseFixture(t, bfaFixture).Genesis
	location, ok := g.Global(gsBridgeLoc)
	if !ok {
		t.Fatal("bridgeLocation global missing")
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

func TestParseSyntheticTransferMint(t *testing.T) {
	info := parseFixture(t, transferFixture)
	if !info.IsTransfer() || len(info.Witnesses) != 2 || info.BundleCount != 2 {
		t.Fatalf("kind=%q witnesses=%d bundles=%d", info.Kind, len(info.Witnesses), info.BundleCount)
	}
	mint := info.Witnesses[0].Transitions[0]
	if mint.TransitionType != tsMint {
		t.Fatalf("transition type = %d, want TS_MINT", mint.TransitionType)
	}
	expectU64(t, "issuedSupply", mint.Global, gsIssuedSupply, 1000)
	expectU64(t, "afterBlock", mint.Meta, msAfterBlock, 42)
	if len(mint.FungibleAllocations) != 1 || mint.FungibleAllocations[0].Total != 1000 {
		t.Errorf("mint allocations = %+v", mint.FungibleAllocations)
	}
}

func TestParseSyntheticTransferBurn(t *testing.T) {
	info := parseFixture(t, transferFixture)
	burn := info.Witnesses[1].Transitions[0]
	if burn.TransitionType != tsBurn {
		t.Fatalf("transition type = %d, want TS_BURN", burn.TransitionType)
	}
	expectU64(t, "burnedAsset", burn.Global, gsBurnedAsset, 400)
	reason, ok := burn.Global(gsBurnReason)
	want := append(make([]byte, 12), bytes.Repeat([]byte{0xab}, 20)...)
	if !ok || !bytes.Equal(reason, want) {
		t.Errorf("burnReason = %x, ok=%v, want %x", reason, ok, want)
	}
	change := burn.FungibleAllocations[0].Entries[0]
	if change.Amount != 600 || change.Seal.Kind != SealConfidential || change.Seal.SecretSeal == "" {
		t.Errorf("change = %+v", change)
	}
}

func TestParseSyntheticTransferTerminals(t *testing.T) {
	info := parseFixture(t, transferFixture)
	if len(info.Terminals) != 1 || len(info.Terminals[0].Seals) != 2 {
		t.Fatalf("terminals = %+v", info.Terminals)
	}
	concealed, revealed := info.Terminals[0].Seals[0], info.Terminals[0].Seals[1]
	if concealed.Kind != SealConfidential || revealed.Kind != SealRevealed || revealed.Vout != 2 {
		t.Errorf("terminal seals = %+v / %+v", concealed, revealed)
	}
	change := info.Witnesses[1].Transitions[0].FungibleAllocations[0].Entries[0]
	strEq(t, "concealed terminal", concealed.SecretSeal, change.Seal.SecretSeal)
	// The revealed terminal has no txid: it lives in the burn bundle's witness tx.
	strEq(t, "terminal bundle", info.Terminals[0].BundleID, info.Witnesses[1].BundleID)
	if info.Witnesses[0].BundleID == info.Witnesses[1].BundleID {
		t.Errorf("mint and burn witnesses share bundle id %s", info.Witnesses[0].BundleID)
	}
}

func TestParseRejectsLegacyV0Transfer(t *testing.T) {
	_, err := ParseFile(filepath.FromSlash(legacyV0Fixture))
	if err == nil || !strings.Contains(err.Error(), "ConsignmentVer") {
		t.Fatalf("legacy V0 transfer must be rejected on its version, got err=%v", err)
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
