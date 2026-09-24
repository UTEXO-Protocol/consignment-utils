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
statically, so the resulting binary has no runtime dependency on Rust. The
archive is built by `scripts/build-lib.sh` (or `make rust`) and is **not**
committed; CI builds one per platform and attaches them to tagged releases.

To use the module locally, build the archive for your host first:

```sh
make rust          # cargo build --release --lib --no-default-features + copy
make test          # cargo test + go test ./rgbconsignment
```

Supported targets: `darwin_arm64`, `darwin_amd64`, `linux_arm64`, `linux_amd64`.

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
