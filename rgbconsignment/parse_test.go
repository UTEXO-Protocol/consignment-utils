package rgbconsignment

import (
	"encoding/json"
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
			raw:  `{"kind":"transfer","version":3,"schema_id":"s","bundle_count":0,"script_count":0,"types_count":0,"genesis":{"contract_id":"c","schema_id":"s","chain_net":"bc","timestamp":1,"issuer":"","ffv":"RGB/1.0","global_state_count":0,"assignment_count":0,"fungible_allocations":[]}}`,
			kind: KindTransfer,
		},
		{
			raw:  `{"kind":"kit","version":3,"schema_count":1,"script_count":0,"types_count":0}`,
			kind: KindKit,
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
	raw := `{"op_id":"op","transition_type":8010,"input_count":1,"fungible_allocations":[],` +
		`"metadata":[{"meta_type":1001,"value_hex":"1027000000000000"},{"meta_type":1003,"value_hex":"` +
		"000000000000000000000000abababababababababababababababababababab" + `"}]}`
	var tr TransitionInfo
	if err := json.Unmarshal([]byte(raw), &tr); err != nil {
		t.Fatal(err)
	}
	amount, ok := tr.Meta(1001)
	if !ok || len(amount) != 8 || amount[0] != 0x10 || amount[1] != 0x27 {
		t.Fatalf("MS_BURNED_ASSET bytes = %x, ok=%v", amount, ok)
	}
	recipient, ok := tr.Meta(1003)
	if !ok || len(recipient) != 32 || recipient[12] != 0xab {
		t.Fatalf("MS_BURN_RECIPIENT bytes = %x, ok=%v", recipient, ok)
	}
	if _, ok := tr.Meta(9999); ok {
		t.Fatal("unknown meta type must not be found")
	}

	var none TransitionInfo
	if err := json.Unmarshal([]byte(`{"op_id":"x","transition_type":10000,"input_count":0,"fungible_allocations":[]}`), &none); err != nil {
		t.Fatal(err)
	}
	if len(none.Metadata) != 0 {
		t.Fatalf("expected no metadata, got %+v", none.Metadata)
	}
}
