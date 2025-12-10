#!/bin/bash

n=10000000
rs=20
d=1024
cargo build --release --bin snapshot
RUST_LOG=trace ../target/release/snapshot -d $d -n $n -r $rs uffd -o uffd.json
RUST_LOG=trace ../target/release/snapshot -d $d -n $n -r $rs soft-dirty -o soft_dirty.json
RUST_LOG=trace ../target/release/snapshot -d $d -n $n -r $rs emulated-soft-dirty -o esd.json
python3 ../scripts/pmscan.py uffd.json soft_dirty.json esd.json 
