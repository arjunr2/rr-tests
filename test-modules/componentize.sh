#!/bin/bash

# Generates components from all componentizable-core modules

meta=tmp.wat

mkdir -p components/wit

for core in core-componentizable/*; do
    base=$(basename ${core%%.*})
    base_wit=wit/$base.wit
    component=components/$base.wat
    new_wit=components/wit/$base.wit
    echo "Processing '$core' with '$base_wit'"
    set -x
    wasm-tools component embed $base_wit $core -t -o $meta
    wasm-tools component new $meta -t -o $component
    wasm-tools component wit $component -o $new_wit
    set +x
    rm $meta
done
