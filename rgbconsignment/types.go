package rgbconsignment

import (
	"encoding/hex"
	"errors"
	"fmt"
)

// Kind is the top-level consignment container type.
type Kind string

const (
	KindTransfer Kind = "transfer"
	KindContract Kind = "contract"
)

// Info is the flat summary returned by Parse. Transfer and Contract share
// the same fields; a contract has no terminals or witnesses.
//
// Info is a decode target for the parser's JSON, not a wire format of its
// own: re-marshaling an Info is not guaranteed to deserialize back into the
// Rust ConsignmentInfo.
type Info struct {
	Kind Kind `json:"kind"`

	// Version is the container version, 1 for the V1 containers of RGB 0.11.2.
	Version     uint8  `json:"version"`
	SchemaID    string `json:"schema_id"`
	BundleCount uint32 `json:"bundle_count"`

	Genesis   *GenesisInfo   `json:"genesis,omitempty"`
	Terminals []TerminalInfo `json:"terminals,omitempty"`
	Witnesses []WitnessInfo  `json:"witnesses,omitempty"`
}

func (i Info) IsTransfer() bool { return i.Kind == KindTransfer }
func (i Info) IsContract() bool { return i.Kind == KindContract }

type GenesisInfo struct {
	ContractID          string               `json:"contract_id"`
	SchemaID            string               `json:"schema_id"`
	ChainNet            string               `json:"chain_net"`
	Timestamp           int64                `json:"timestamp"`
	Issuer              string               `json:"issuer"`
	FFV                 string               `json:"ffv"`
	GlobalStateCount    uint32               `json:"global_state_count"`
	AssignmentCount     uint32               `json:"assignment_count"`
	FungibleAllocations []FungibleAllocation `json:"fungible_allocations"`
	Name                *string              `json:"name,omitempty"`
	Ticker              *string              `json:"ticker,omitempty"`
	Precision           *uint8               `json:"precision,omitempty"`
	Details             *string              `json:"details,omitempty"`
	// Globals is the global state declared at genesis, one entry per value
	// (for BFA: bridgeLocation=3008).
	Globals []GlobalEntry `json:"globals,omitempty"`
}

// Global returns the raw bytes of the first genesis value of globalType, if present.
func (g GenesisInfo) Global(globalType uint16) ([]byte, bool) {
	return firstGlobal(g.Globals, globalType)
}

type FungibleAllocation struct {
	AssignmentType uint16          `json:"assignment_type"`
	Entries        []FungibleEntry `json:"entries"`
	Total          uint64          `json:"total"`
}

type FungibleEntry struct {
	Amount uint64 `json:"amount"`
	Seal   Seal   `json:"seal"`
}

type SealKind string

const (
	SealRevealed     SealKind = "revealed"
	SealConfidential SealKind = "confidential"
)

type Seal struct {
	Kind       SealKind `json:"kind"`
	Txid       *string  `json:"txid,omitempty"`
	Vout       uint32   `json:"vout"`
	SecretSeal string   `json:"secret_seal,omitempty"`
}

type TransitionInfo struct {
	OpID                string               `json:"op_id"`
	TransitionType      uint16               `json:"transition_type"`
	InputCount          uint32               `json:"input_count"`
	FungibleAllocations []FungibleAllocation `json:"fungible_allocations"`
	// Metadata carries the transition's schema-defined metadata fields
	// (for a BFA mint: afterBlock=1003) as raw strict-encoded bytes. Empty
	// when the transition has none.
	Metadata []MetaEntry `json:"metadata,omitempty"`
	// Globals carries the global state the transition commits, one entry
	// per value (for a BFA burn: burnedAsset=2015, burnReason=3009). Empty
	// when the transition has none.
	Globals []GlobalEntry `json:"globals,omitempty"`
}

// MetaEntry is one metadata field of a transition.
type MetaEntry struct {
	MetaType uint16 `json:"meta_type"`
	// ValueHex is the raw strict-encoded value, lowercase hex without 0x.
	ValueHex string `json:"value_hex"`
}

// Meta returns the raw bytes of the metadata field metaType, if present.
func (t TransitionInfo) Meta(metaType uint16) ([]byte, bool) {
	for _, m := range t.Metadata {
		if m.MetaType == metaType {
			b, err := hex.DecodeString(m.ValueHex)
			if err != nil {
				return nil, false
			}
			return b, true
		}
	}
	return nil, false
}

// GlobalEntry is one global state value of an operation.
type GlobalEntry struct {
	GlobalType uint16 `json:"global_type"`
	// ValueHex is the raw strict-encoded value, lowercase hex without 0x.
	ValueHex string `json:"value_hex"`
}

// Global returns the raw bytes of the first value of globalType, if present.
// It does not notice repeats; use GlobalOnce for a type the schema caps at one.
func (t TransitionInfo) Global(globalType uint16) ([]byte, bool) {
	return firstGlobal(t.Globals, globalType)
}

// ErrGlobalNotFound reports a transition without a value of the requested global type.
var ErrGlobalNotFound = errors.New("global not found")

// GlobalOnce returns the raw bytes of the only value of globalType. It fails on a
// repeat because the parser does not validate the schema's Once / NoneOrOnce rule.
func (t TransitionInfo) GlobalOnce(globalType uint16) ([]byte, error) {
	var values []string
	for _, g := range t.Globals {
		if g.GlobalType == globalType {
			values = append(values, g.ValueHex)
		}
	}
	switch len(values) {
	case 0:
		return nil, fmt.Errorf("transition %s: global %d: %w", t.OpID, globalType, ErrGlobalNotFound)
	case 1:
		b, err := hex.DecodeString(values[0])
		if err != nil {
			return nil, fmt.Errorf("transition %s: global %d is not valid hex: %w", t.OpID, globalType, err)
		}
		return b, nil
	default:
		return nil, fmt.Errorf("transition %s: global %d appears %d times, the schema allows one",
			t.OpID, globalType, len(values))
	}
}

func firstGlobal(globals []GlobalEntry, globalType uint16) ([]byte, bool) {
	for _, g := range globals {
		if g.GlobalType == globalType {
			b, err := hex.DecodeString(g.ValueHex)
			if err != nil {
				return nil, false
			}
			return b, true
		}
	}
	return nil, false
}

// TerminalInfo lists the terminal seals of one bundle: concealed for blinded
// receives, revealed for witness-vout ones. A revealed seal without a Txid sits
// in the tx of the WitnessInfo with the same BundleID.
type TerminalInfo struct {
	BundleID string `json:"bundle_id"`
	Seals    []Seal `json:"seals"`
}

type WitnessInfo struct {
	Txid            string           `json:"txid"`
	BundleID        string           `json:"bundle_id"`
	TransitionCount uint32           `json:"transition_count"`
	InputCount      uint32           `json:"input_count"`
	Transitions     []TransitionInfo `json:"transitions"`
}
