# rgb-consignment

Parses binary RGB consignment files (`.rgb`: Transfer, Contract or Kit) into a
flat summary: genesis, mint/transfer transitions with their fungible
allocations, terminals and witness transactions.

Three entry points share one parser:

| Entry point | Where |
|---|---|
| Rust crate `rgb_consignment::parse` | `src/` |
| CLI `rgb-consignment <file.rgb>` (JSON to stdout) | `src/main.rs` |
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

`testdata/ifa-contract.rgb` is `ifa-example.rgb` from the `rgb-schemas` crate
(Apache-2.0). It is used by both `tests/fixtures.rs` and
`rgbconsignment/fixtures_test.go`.
