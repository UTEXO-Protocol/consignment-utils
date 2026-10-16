package rgbconsignment

import (
	"encoding/json"
	"errors"
	"strings"
	"testing"
)

func TestParseRejectsGarbage(t *testing.T) {
	_, err := Parse([]byte("not a consignment"))
	if err == nil {
		t.Fatal("expected parse error")
	}
}

func TestParseRejectsEmpty(t *testing.T) {
	_, err := Parse(nil)
	if err == nil {
		t.Fatal("expected parse error")
	}
}

func TestJSONKinds(t *testing.T) {
	cases := []struct {
		raw  string
		kind Kind
	}{
		{
			raw:  `{"kind":"transfer","version":1,"schema_id":"s","bundle_count":0,"genesis":{"contract_id":"c","schema_id":"s","chain_net":"bc","timestamp":1,"issuer":"","ffv":"RGB/1.0","global_state_count":0,"assignment_count":0,"fungible_allocations":[]},"terminals":[],"witnesses":[]}`,
			kind: KindTransfer,
		},
		{
			raw:  `{"kind":"contract","version":1,"schema_id":"s","bundle_count":0,"genesis":{"contract_id":"c","schema_id":"s","chain_net":"bc","timestamp":1,"issuer":"","ffv":"RGB/1.0","global_state_count":0,"assignment_count":0,"fungible_allocations":[]},"terminals":[],"witnesses":[]}`,
			kind: KindContract,
		},
	}

	for _, tc := range cases {
		var info Info
		if err := json.Unmarshal([]byte(tc.raw), &info); err != nil {
			t.Fatalf("%s: %v", tc.kind, err)
		}
		if info.Kind != tc.kind {
			t.Fatalf("got %q, want %q", info.Kind, tc.kind)
		}
	}
}

func TestTransitionMetadataDecodesAndLooksUp(t *testing.T) {
	raw := `{"op_id":"op","transition_type":8013,"input_count":1,"fungible_allocations":[],` +
		`"metadata":[{"meta_type":1003,"value_hex":"2a00000000000000"}]}`
	var tr TransitionInfo
	if err := json.Unmarshal([]byte(raw), &tr); err != nil {
		t.Fatal(err)
	}
	afterBlock, ok := tr.Meta(1003)
	if !ok || len(afterBlock) != 8 || afterBlock[0] != 0x2a {
		t.Fatalf("afterBlock bytes = %x, ok=%v", afterBlock, ok)
	}
	if _, ok := tr.Meta(9999); ok {
		t.Fatal("unknown meta type must not be found")
	}
}

func TestTransitionGlobalsDecodeAndLookUp(t *testing.T) {
	raw := `{"op_id":"op","transition_type":8010,"input_count":1,"fungible_allocations":[],` +
		`"globals":[{"global_type":2015,"value_hex":"9001000000000000"},{"global_type":3009,"value_hex":"` +
		"000000000000000000000000abababababababababababababababababababab" + `"}]}`
	var tr TransitionInfo
	if err := json.Unmarshal([]byte(raw), &tr); err != nil {
		t.Fatal(err)
	}
	amount, ok := tr.Global(2015)
	if !ok || len(amount) != 8 || amount[0] != 0x90 || amount[1] != 0x01 {
		t.Fatalf("burnedAsset bytes = %x, ok=%v", amount, ok)
	}
	reason, ok := tr.Global(3009)
	if !ok || len(reason) != 32 || reason[11] != 0 || reason[12] != 0xab {
		t.Fatalf("burnReason bytes = %x, ok=%v", reason, ok)
	}
	if _, ok := tr.Global(2010); ok {
		t.Fatal("absent global type must not be found")
	}
}

func TestGlobalReturnsFirstValueOfType(t *testing.T) {
	g := GenesisInfo{Globals: []GlobalEntry{
		{GlobalType: 2010, ValueHex: "01"},
		{GlobalType: 2010, ValueHex: "02"},
	}}
	b, ok := g.Global(2010)
	if !ok || len(b) != 1 || b[0] != 0x01 {
		t.Fatalf("Global(2010) = %x, ok=%v, want first value 01", b, ok)
	}
}

func TestGlobalOnceReturnsTheOnlyValue(t *testing.T) {
	tr := TransitionInfo{Globals: []GlobalEntry{
		{GlobalType: 2015, ValueHex: "9001000000000000"},
		{GlobalType: 3009, ValueHex: "ab"},
	}}
	b, err := tr.GlobalOnce(2015)
	if err != nil || len(b) != 8 || b[0] != 0x90 {
		t.Fatalf("GlobalOnce(2015) = %x, err=%v", b, err)
	}
}

func TestGlobalOnceRejectsRepeatedType(t *testing.T) {
	tr := TransitionInfo{OpID: "op", Globals: []GlobalEntry{
		{GlobalType: 2015, ValueHex: "9001000000000000"},
		{GlobalType: 2015, ValueHex: "0100000000000000"},
	}}
	b, err := tr.GlobalOnce(2015)
	if err == nil || !strings.Contains(err.Error(), "appears 2 times") {
		t.Fatalf("two burnedAsset values must be rejected, got %x, err=%v", b, err)
	}
	if errors.Is(err, ErrGlobalNotFound) {
		t.Fatalf("a repeat is not a missing global: %v", err)
	}
}

func TestGlobalOnceReportsMissingType(t *testing.T) {
	tr := TransitionInfo{OpID: "op", Globals: []GlobalEntry{{GlobalType: 2015, ValueHex: "00"}}}
	if _, err := tr.GlobalOnce(3009); !errors.Is(err, ErrGlobalNotFound) {
		t.Fatalf("err = %v, want ErrGlobalNotFound", err)
	}
}

func TestGlobalOnceRejectsMalformedHex(t *testing.T) {
	tr := TransitionInfo{OpID: "op", Globals: []GlobalEntry{{GlobalType: 3009, ValueHex: "zz"}}}
	if b, err := tr.GlobalOnce(3009); err == nil || errors.Is(err, ErrGlobalNotFound) {
		t.Fatalf("malformed hex must fail as such, got %x, err=%v", b, err)
	}
}

func TestGenesisGlobalOnceReturnsTheOnlyValue(t *testing.T) {
	g := GenesisInfo{Globals: []GlobalEntry{
		{GlobalType: 2000, ValueHex: "00"},
		{GlobalType: 3008, ValueHex: "0001"},
	}}
	b, err := g.GlobalOnce(3008)
	if err != nil || len(b) != 2 || b[1] != 0x01 {
		t.Fatalf("GlobalOnce(3008) = %x, err=%v", b, err)
	}
}

func TestGenesisGlobalOnceRejectsRepeatedType(t *testing.T) {
	g := GenesisInfo{ContractID: "rgb:c", Globals: []GlobalEntry{
		{GlobalType: 3008, ValueHex: "00"},
		{GlobalType: 3008, ValueHex: "01"},
	}}
	b, err := g.GlobalOnce(3008)
	if err == nil || !strings.Contains(err.Error(), "genesis rgb:c: global 3008 appears 2 times") {
		t.Fatalf("two bridgeLocation values must be rejected, got %x, err=%v", b, err)
	}
	if errors.Is(err, ErrGlobalNotFound) {
		t.Fatalf("a repeat is not a missing global: %v", err)
	}
}

func TestGenesisGlobalOnceReportsMissingType(t *testing.T) {
	g := GenesisInfo{ContractID: "rgb:c", Globals: []GlobalEntry{{GlobalType: 2000, ValueHex: "00"}}}
	if _, err := g.GlobalOnce(3008); !errors.Is(err, ErrGlobalNotFound) {
		t.Fatalf("err = %v, want ErrGlobalNotFound", err)
	}
}

func TestGenesisGlobalOnceRejectsMalformedHex(t *testing.T) {
	g := GenesisInfo{Globals: []GlobalEntry{{GlobalType: 3008, ValueHex: "zz"}}}
	if b, err := g.GlobalOnce(3008); err == nil || errors.Is(err, ErrGlobalNotFound) {
		t.Fatalf("malformed hex must fail as such, got %x, err=%v", b, err)
	}
}

func TestGlobalRejectsMalformedHex(t *testing.T) {
	tr := TransitionInfo{Globals: []GlobalEntry{{GlobalType: 2015, ValueHex: "zz"}}}
	if b, ok := tr.Global(2015); ok {
		t.Fatalf("malformed hex must not decode, got %x", b)
	}
}

func TestEmptyGlobalsAndMetadataDecodeAsNil(t *testing.T) {
	var none TransitionInfo
	if err := json.Unmarshal([]byte(`{"op_id":"x","transition_type":10000,"input_count":0,"fungible_allocations":[]}`), &none); err != nil {
		t.Fatal(err)
	}
	if len(none.Metadata) != 0 || len(none.Globals) != 0 {
		t.Fatalf("expected no metadata or globals, got %+v / %+v", none.Metadata, none.Globals)
	}
}
