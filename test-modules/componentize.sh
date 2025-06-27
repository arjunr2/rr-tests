#!/bin/bash

# Generates components from all componentizable-core modules

meta=tmp.wat

for core in core-componentizable/*; do
    base=$(basename ${core%%.*})
    wit=wit/$base.wit
    component=components/$base.wat
    echo "Processing '$core' with '$wit'"
    set -x
    wasm-tools component embed $wit $core -t -o $meta
    wasm-tools component new $meta -t -o $component
    set +x
    rm $meta
done
