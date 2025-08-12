RUST_LIBS=$(wildcard rust-componentizable/*)
RUST_TOMLS=$(addsuffix /Cargo.toml,$(RUST_LIBS))

RUST_TARGET_PATH=target/wasm32-wasip1/release

.PHONY: all clean $(RUST_TOMLS) $(RUST_LIBS)

components: core rust

core:
	./core-componentize.sh

.ONESHELL:
rust: $(RUST_LIBS)

.ONESHELL:
$(RUST_LIBS):
	cargo component build --release --manifest-path=$@/Cargo.toml
	set -x
	wasm-tools parse -t $@/$(RUST_TARGET_PATH)/`basename $@`.wasm -o components/`basename $@`.wat
	cp $@/wit/*.wit components/wit/`basename $@`.wit

clean:
	rm -r components
