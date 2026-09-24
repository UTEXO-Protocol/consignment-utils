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
