package rgbconsignment

// Kind is the top-level consignment container type.
type Kind string

const (
	KindTransfer Kind = "transfer"
	KindContract Kind = "contract"
	KindKit      Kind = "kit"
)

// Info is the flat summary returned by Parse. Transfer and Contract share
// the same fields; Kit only fills Version / SchemaCount / ScriptCount /
// TypesCount.
type Info struct {
	Kind Kind `json:"kind"`

	Version     uint8  `json:"version"`
	SchemaID    string `json:"schema_id,omitempty"`
	BundleCount uint32 `json:"bundle_count,omitempty"`
	ScriptCount uint32 `json:"script_count"`
	TypesCount  uint32 `json:"types_count"`
	SchemaCount uint32 `json:"schema_count,omitempty"`

	Genesis   *GenesisInfo   `json:"genesis,omitempty"`
	Terminals []TerminalInfo `json:"terminals,omitempty"`
	Witnesses []WitnessInfo  `json:"witnesses,omitempty"`
}

func (i Info) IsTransfer() bool { return i.Kind == KindTransfer }
func (i Info) IsContract() bool { return i.Kind == KindContract }
func (i Info) IsKit() bool      { return i.Kind == KindKit }

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
	Vout       uint32   `json:"vout,omitempty"`
	SecretSeal string   `json:"secret_seal,omitempty"`
}

type TransitionInfo struct {
	OpID                string               `json:"op_id"`
	TransitionType      uint16               `json:"transition_type"`
	InputCount          uint32               `json:"input_count"`
	FungibleAllocations []FungibleAllocation `json:"fungible_allocations"`
}

type TerminalInfo struct {
	BundleID        string `json:"bundle_id"`
	SecretSealCount uint32 `json:"secret_seal_count"`
}

type WitnessInfo struct {
	Txid            string           `json:"txid"`
	TransitionCount uint32           `json:"transition_count"`
	InputCount      uint32           `json:"input_count"`
	Transitions     []TransitionInfo `json:"transitions"`
}
