.PHONY: rust go test

GOOS ?= $(shell go env GOOS)
GOARCH ?= $(shell go env GOARCH)
LIB := rgbconsignment/lib/$(GOOS)_$(GOARCH)/librgb_consignment.a
RUST_SOURCES := $(wildcard src/*.rs) Cargo.toml Cargo.lock

rust: $(LIB)

$(LIB): $(RUST_SOURCES)
	./scripts/build-lib.sh

go: rust
	go test ./rgbconsignment

test: rust
	cargo test
	go test ./rgbconsignment
