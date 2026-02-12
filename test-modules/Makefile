RUST_LIBS=$(wildcard rust-componentizable/*)
RUST_TOMLS=$(addsuffix /Cargo.toml,$(RUST_LIBS))
WASI_SRCS=$(wildcard wasi/*.rs)

RUST_TARGET_PATH=target/wasm32-wasip1/debug

$(info $(WASI_SRCS))

.PHONY: all clean wasi $(RUST_TOMLS) $(RUST_LIBS)

all: core rust wasi

wasi:
	cargo build --target=wasm32-wasip2 --manifest-path=wasi/Cargo.toml

core:
	./core-componentize.sh

.ONESHELL:
rust: $(RUST_LIBS)

.ONESHELL:
$(RUST_LIBS):
	cargo component build --manifest-path=$@/Cargo.toml
	set -x
	wasm-tools parse -t $@/$(RUST_TARGET_PATH)/`basename $@`.wasm -o components/`basename $@`.wat
	cp $@/wit/*.wit components/wit/`basename $@`.wit

clean:
	rm -r components
