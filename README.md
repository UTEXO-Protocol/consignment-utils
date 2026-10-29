# rgb-consignment

Parses binary RGB consignment files into a flat summary: genesis, mint /
transfer / burn transitions with their fungible allocations, metadata and
global state, and witness transactions.

Built on RGB **0.11.2-rc.1** at the revs `rgb-tools-devs/rgb-lib` commit
`f3975623` patches in (`rgb-consensus 4a614b72`, `rgb-ops 9d831d2`,
`rgb-schemas aa1e7fd`, the strict encoding / types revs; see
`[patch.crates-io]` in `Cargo.toml`). It reads the V1 containers they write:

| Magic | Container | Written by rgb-lib as |
|---|---|---|
| `RGB\0TFR` | Transfer | `consignment_out`, received `rcv_compose.rgbc` |
| `RGB\0CON` | Contract | - (rgb-ops `Contract::save`) |
| `RGB\0VTF` | Validated transfer | `*.valid.rgbc` |
| `RGB\0VCO` | Validated contract | the issued asset file `assets/<asset_id>` |

The validated containers append their validation status as JSON; it is not
read. Legacy V0 consignments (RGB 0.11.1 and older) and schema definition
files (`SD2`) are rejected. So are V1 files in the layout with history
terminals that rgb-ops dropped in `596d3cb` without a version bump (e.g.
written by rgb-lib before `f3975623`): they fail with
`ParseError::LegacyTerminals` instead of an unrelated decode error. A V1
consignment carries no schema or
type system, so asset name, ticker, precision and details are decoded against
the known `rgb-ops` types (`AssetSpec`, or the root-level `Name` /
`Precision` / `Details` of CFA and BFA, plus the BFA `UnicodeTicker`);
nothing is validated.

The `rgb-tools-devs` repos are private. Locally, with HTTPS credentials for
them, map the ssh URLs Cargo is given:

```sh
export CARGO_NET_GIT_FETCH_WITH_CLI=true GIT_CONFIG_COUNT=1 \
  GIT_CONFIG_KEY_0=url.https://github.com/rgb-tools-devs/.insteadOf \
  GIT_CONFIG_VALUE_0=ssh://git@github.com/rgb-tools-devs/
```

Without access to `rgb-tools-devs`, map them to our
`UTEXO-Protocol/<repo>-s-bfa` mirrors instead, as CI does with the
`RGB_PRIVATE_DEPS_TOKEN` secret:

```sh
export CARGO_NET_GIT_FETCH_WITH_CLI=true
for r in rgb-consensus rgb-ops rgb-api rgb-schemas; do
  git config --global url."https://github.com/UTEXO-Protocol/$r-s-bfa.git".insteadOf \
    "ssh://git@github.com/rgb-tools-devs/$r.git"
done
```

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
beforeBlock, ok := mint.Meta(1003)        // u64 LE
location, err := info.Genesis.GlobalOnce(3008)
```

A revealed seal without a `txid` points into the witness tx of the
`WitnessInfo` holding its transition.

A consignment no longer names its beneficiaries (rgb-ops `596d3cb` dropped the
terminals): a receiver finds what it is paid by matching its own seals against
the `FungibleEntry` seals.

BFA ids in rgb-schemas `aa1e7fd` (`src/lib.rs`):

| Id | Name | Value layout |
|---|---|---|
| 8013 | `TS_MINT` | spends one `mintRight`; globals `issuedSupply` 2010 (u64 LE), metadata `beforeBlock` 1003 (u64 LE) |
| 8014 | `TS_PROPAGATE` | spends one `mintRight`, assigns one or more |
| 8010 | `TS_BURN` | no assignments; globals `burnedAsset` 2015 (u64 LE), `burnReason` 3009 (32 bytes), both optional |
| 10000 | `TS_TRANSFER` | |
| 8016 | `TS_TRANSFER_LINK` | moves the `linkRight` |
| 4000 | `OS_ASSET` | fungible |
| 4013 | `OS_MINT` | declarative (`mintRight`) |
| 3008 | `GS_BRIDGE_LOCATION` | genesis: tag `00` (Evm), chain id u64 LE, 20-byte address |
| 3001, 3004, 3005, 3011 | `GS_NAME`, `GS_DETAILS`, `GS_PRECISION`, `GS_TICKER` | genesis nominals, reported as `Name`, `Details`, `Precision`, `Ticker` |

A burn burns whole allocations, so it has no change. When the inputs exceed
the burned amount, rgb-lib first splits them with a `TS_TRANSFER` in the same
bundle, paying the change and putting the burned amount on an outpoint the
burn tx spends, and the `TS_BURN` spends that output; the split precedes the
burn. An exact burn is a `TS_BURN` spending its inputs directly. Transition
types the parser has no name for (e.g. IFA's `TS_TRANSFER_INFLATION` 8015) are
reported by number like any other.

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

## Upgrading from v0.2.0-rc.2

- Go `Info.Terminals` / `TerminalInfo` and Rust `TerminalInfo` are gone: match
  your own seals against the `FungibleEntry` seals instead.
- `ParseError::LegacyTerminals` is a new variant. Copies of the old `testdata`
  files fail with it; take the new ones.
- BFA mint metadata 1003 is `beforeBlock` (was `afterBlock`), same id and u64
  LE layout.

## Test fixtures

`tests/synthetic` builds four of them with the pinned `rgb-schemas` (a
dev-dependency) and `rgb-ops`:

- `testdata/{bfa,ifa,cfa}-contract.rgb` are issued and validated by the
  rgb-ops contract builder with the inputs of the `rgb-schemas` examples
  (`examples/{bfa,ifa,cfa}.rs`, Apache-2.0; the BFA one also sets `details`),
  in `VCO` containers. The `test/*-example.rgb` files in `rgb-schemas aa1e7fd`
  are stale (the layout with terminals, and older BFA and IFA schema ids),
  hence the rebuild.
- `testdata/bfa-transfer-synthetic.rgb` is a BFA burn transfer shaped as
  rgb-lib `f3975623` composes one: a `TS_PROPAGATE` of a genesis right, a
  `TS_MINT` spending it, then one bundle with the split `TS_TRANSFER` and the
  `TS_BURN`. Its anchors are dummies: it exercises the parser, not
  validation, and is no substitute for transfers produced by a release wallet.

After changing the builder, regenerate them with
`cargo test --test fixtures regenerate_fixtures -- --ignored`;
`committed_fixtures_match_builder` fails until you do.

Negative fixtures:

- `testdata/legacy-terminals-bfa-{contract,transfer}.rgb` are the previous
  fixtures, V1 with terminals (rgb-ops before `596d3cb`). The tests assert
  `ParseError::LegacyTerminals`.
- `testdata/legacy-v0-bfa-burn.rgb` is a V0 BFA burn transfer from the
  pre-cutover rgb-lib fork (enclave-signer `bfa_burn_consignment.rgbc`). The
  tests assert it is rejected, which the hard cutover relies on.
