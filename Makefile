.PHONY: rust go test libs

LIB := rgbconsignment/lib/$(shell go env GOHOSTOS)_$(shell go env GOHOSTARCH)/librgb_consignment.a
RUST_SOURCES := $(wildcard src/*.rs) Cargo.toml Cargo.lock

# Host-platform archive only; CI rebuilds and commits all four platforms.
rust: $(LIB)

$(LIB): $(RUST_SOURCES)
	./scripts/build-lib.sh

go: rust
	go test ./rgbconsignment

test: rust
	cargo test
	go test ./rgbconsignment
