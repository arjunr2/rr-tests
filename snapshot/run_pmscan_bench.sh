#!/bin/bash

n=1000000
rs=100
d=16384
cargo build --release --bin snapshot
RUST_LOG=trace ../target/release/snapshot -d $d -n $n -r $rs emulated-soft-dirty -o esd.json
RUST_LOG=trace ../target/release/snapshot -d $d -n $n -r $rs uffd -o uffd.json
python3 ../scripts/pmscan.py esd.json uffd.json
