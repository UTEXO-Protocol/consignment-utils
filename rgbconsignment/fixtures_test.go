package rgbconsignment

import (
	"os"
	"path/filepath"
	"testing"
)

const ifaFixture = "../testdata/ifa-contract.rgb"

func TestParseFileIFAContract(t *testing.T) {
	info, err := ParseFile(filepath.FromSlash(ifaFixture))
	if err != nil {
		t.Fatalf("ParseFile: %v", err)
	}

	if !info.IsContract() || info.Kind != KindContract {
		t.Fatalf("kind = %q, want %q", info.Kind, KindContract)
	}
	if info.Genesis == nil {
		t.Fatal("genesis is nil")
	}
	g := info.Genesis

	strEq(t, "contract_id", g.ContractID, "rgb:jpyfP_3m-zroPnJm-2J9qFHO-7ZjimUC-yH4A96u-oHthc64")
	strEq(t, "schema_id", g.SchemaID, "rgb:sch:p6H_wtDgei9HHUVLjKW0tNdHHFLhfHxrn9QX_QQUE78#scale-year-shave")
	strEq(t, "chain_net", g.ChainNet, "tb4")
	strEq(t, "issuer", g.Issuer, "ssi:anonymous")
	strEq(t, "ffv", g.FFV, "RGB/1.0")
	if g.Timestamp != 1768417034 {
		t.Errorf("timestamp = %d", g.Timestamp)
	}
	if g.Name == nil || *g.Name != "Test asset" {
		t.Errorf("name = %v", g.Name)
	}
	if g.Ticker == nil || *g.Ticker != "TEST" {
		t.Errorf("ticker = %v", g.Ticker)
	}
	if g.Precision == nil || *g.Precision != 8 {
		t.Errorf("precision = %v", g.Precision)
	}
	if g.Details != nil {
		t.Errorf("details = %q, want nil", *g.Details)
	}
	if g.GlobalStateCount != 5 || g.AssignmentCount != 2 {
		t.Errorf("counts = %d/%d, want 5/2", g.GlobalStateCount, g.AssignmentCount)
	}

	if len(g.FungibleAllocations) != 2 {
		t.Fatalf("fungible_allocations len = %d, want 2", len(g.FungibleAllocations))
	}
	a := g.FungibleAllocations[0]
	if a.AssignmentType != 4000 || a.Total != 100000 || len(a.Entries) != 1 {
		t.Errorf("alloc[0] = %+v", a)
	}
	e := a.Entries[0]
	if e.Amount != 100000 || e.Seal.Kind != SealRevealed || e.Seal.Vout != 1 {
		t.Errorf("entry[0] = %+v", e)
	}
	if e.Seal.Txid == nil || *e.Seal.Txid != "14295d5bb1a191cdb6286dc0944df938421e3dfcbf0811353ccac4100c2068c5" {
		t.Errorf("seal txid = %v", e.Seal.Txid)
	}
	b := g.FungibleAllocations[1]
	if b.AssignmentType != 4010 || b.Total != 50000 {
		t.Errorf("alloc[1] = %+v", b)
	}

	strEq(t, "top-level schema_id", info.SchemaID, g.SchemaID)
	if len(info.Terminals) != 0 || len(info.Witnesses) != 0 || info.BundleCount != 0 {
		t.Errorf("terminals/witnesses/bundles = %d/%d/%d, want 0/0/0",
			len(info.Terminals), len(info.Witnesses), info.BundleCount)
	}
	if info.ScriptCount != 3 || info.TypesCount != 39 {
		t.Errorf("script/types = %d/%d, want 3/39", info.ScriptCount, info.TypesCount)
	}
}

func TestParseRejectsTruncatedFixture(t *testing.T) {
	data, err := os.ReadFile(filepath.FromSlash(ifaFixture))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := Parse(data[:len(data)/2]); err == nil {
		t.Fatal("expected error for truncated consignment")
	}
}

func BenchmarkParseIFAContract(b *testing.B) {
	data, err := os.ReadFile(filepath.FromSlash(ifaFixture))
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
