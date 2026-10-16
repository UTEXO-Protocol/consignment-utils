# rgb-consignment

Parses binary RGB consignment files into a flat summary: genesis, mint /
transfer / burn transitions with their fungible allocations, metadata and
global state, terminals and witness transactions.

Built on RGB **0.11.2-rc.1** (`rgb-ops =0.11.2-rc.1`, `rgb-strict-types
=1.0.5`, `amplify =4.8.1`, the versions the RGB-Tools BFA release of rgb-lib,
`rgb-tools-devs/rgb-lib` commit `3f571ac`, locks). It reads its V1 containers:

| Magic | Container | Written by rgb-lib as |
|---|---|---|
| `RGB\0TFR` | Transfer | `consignment_out`, received `rcv_compose.rgbc` |
| `RGB\0CON` | Contract | - (rgb-ops `Contract::save`) |
| `RGB\0VTF` | Validated transfer | `*.valid.rgbc` |
| `RGB\0VCO` | Validated contract | the issued asset file `assets/<asset_id>` |

The validated containers append their validation status as JSON; it is not
read. Legacy V0 consignments (RGB 0.11.1 and older) and schema definition
files (`SD2`) are rejected. A V1 consignment carries no schema or type
system, so asset name, ticker, precision and details are decoded against the
known `rgb-ops` types (`AssetSpec`, or `Name` / `Precision` / `Details` for
CFA); nothing is validated.

Three entry points share one parser:

| Entry point | Where |
|---|---|
| Rust crate `rgb_consignment::parse` | `src/` |
| CLI `rgb-consignment <file>` (JSON to stdout) | `src/main.rs` |
| Go package `rgbconsignment` (cgo, static link) | `rgbconsignment/` |

## Go binding

```go
import "github.com/UTEXO-Protocol/consignment-utils/rgbconsignment"

info, err := rgbconsignment.Parse(consignmentBytes)
if err != nil { /* invalid consignment */ }
for _, w := range info.Witnesses {
    for _, t := range w.Transitions {
        fmt.Println(t.OpID, t.TransitionType, t.FungibleAllocations)
    }
}
```

### Global state and metadata

Each transition carries its `Metadata []MetaEntry` and `Globals
[]GlobalEntry`, and genesis its `Globals`, as raw strict-encoded bytes in
lowercase hex. `Meta(metaType)` and `Global(globalType)` return the decoded
bytes of the first value of that type. Nothing is validated, so a crafted
genesis or transition can repeat a global the schema caps at one: read those
with `GlobalOnce`, which fails on a repeat and wraps `ErrGlobalNotFound` when
absent.

```go
burnedAsset, err := burn.GlobalOnce(2015) // u64 LE
burnReason, err := burn.GlobalOnce(3009)  // 32 raw bytes, no length prefix
afterBlock, ok := mint.Meta(1003)         // u64 LE
location, err := info.Genesis.GlobalOnce(3008)
```

A revealed seal without a `txid` points into its bundle's witness tx. For a
transition seal that is the enclosing `WitnessInfo`; for a terminal seal it is
a `WitnessInfo` whose `BundleID` equals the terminal's `BundleID`. The parser
keeps every bundle and the bundle id does not depend on the witness tx, so
unvalidated input can match more than one `WitnessInfo`.

BFA ids in rgb-schemas 0.11.2-rc.1 (`src/lib.rs`):

| Id | Name | Value layout |
|---|---|---|
| 8013 | `TS_MINT` | globals `issuedSupply` 2010 (u64 LE), metadata `afterBlock` 1003 (u64 LE) |
| 8010 | `TS_BURN` | globals `burnedAsset` 2015 (u64 LE), `burnReason` 3009 (32 bytes); both optional |
| 10000 | `TS_TRANSFER` | |
| 4000 | `OS_ASSET` | fungible |
| 4013 | `OS_MINT` | declarative (`mintRight`) |
| 3008 | `GS_BRIDGE_LOCATION` | genesis: tag `00` (Evm), chain id u64 LE, 20-byte address |

### Static archives

The Go package links `rgbconsignment/lib/<GOOS>_<GOARCH>/librgb_consignment.a`
statically, so `go get` needs no Rust toolchain and the resulting binary has no
runtime dependency on a shared library. The archives are committed (about
11 MB each, LTO-stripped to the two exported symbols) for `darwin_arm64`,
`darwin_amd64`, `linux_arm64` and `linux_amd64`.

After changing the Rust side, rebuild the host archive with `make rust`
(`scripts/build-lib.sh <target>` cross-compiles the others); CI rebuilds all
four on push and commits them back to the branch, so a local rebuild is only
needed to test.

```sh
make rust          # host archive via cargo rustc --profile lib --crate-type staticlib
make test          # cargo test + go test ./rgbconsignment
```

Linking two Rust static archives into one Go binary can clash on Rust runtime
symbols; this archive exports its Rust runtime like any other staticlib.

### C ABI

`include/rgb_consignment.h` exposes two functions:

- `rgb_consignment_parse(data, len, &err)` returns a JSON document matching the
  Rust `ConsignmentInfo` type, or `NULL` with `err` set.
- `rgb_consignment_string_free(s)` releases either string.

Rust panics never cross the boundary: they are caught and reported through
`err` as `internal error (panic): ...`.

## Test fixtures

- `testdata/{bfa,ifa,cfa}-contract.rgb` are the `*-example.rgb` contracts of
  the `rgb-schemas` 0.11.2-rc.1 crate (Apache-2.0), in `VCO` containers.
- `testdata/bfa-transfer-synthetic.rgb` is a BFA mint + burn transfer built
  by `tests/synthetic` from the rc.1 types and saved as rgb-lib saves one.
  Its anchors are dummies: it exercises the parser, not validation, and is no
  substitute for transfers produced by a release wallet. After changing the
  builder, regenerate it with
  `cargo test --test fixtures regenerate_synthetic_transfer -- --ignored`;
  `committed_synthetic_transfer_matches_builder` fails until you do.
- `testdata/legacy-v0-bfa-burn.rgb` is a V0 BFA burn transfer from the
  pre-cutover rgb-lib fork (enclave-signer `bfa_burn_consignment.rgbc`). The
  tests assert it is rejected, which the hard cutover relies on.
