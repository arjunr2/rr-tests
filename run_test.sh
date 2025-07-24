#!/bin/bash

hyperfine --warmup 3 --shell=none 'wasmtime-release -R path=test.trace --dir=data target/wasm32-wasip2/debug/wasi-compressor.wasm data/uncompressed-1M'
hyperfine --warmup 3 --shell=none 'wasmtime-release --dir=data target/wasm32-wasip2/debug/wasi-compressor.wasm data/uncompressed-1M'
hyperfine --warmup 3 --shell=none 'stock-wasmtime-release --dir=data target/wasm32-wasip2/debug/wasi-compressor.wasm data/uncompressed-1M'
