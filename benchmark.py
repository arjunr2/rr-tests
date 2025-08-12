import subprocess
import argparse

parser = argparse.ArgumentParser(description="Run benchmark for wasmtime with RR support.")
parser.add_argument("--rr", type=str, help="Path to RR-supported Wasmtime", required=True)
parser.add_argument("--upstream", type=str, help="Path to upstream Wasmtime", required=True)

args = parser.parse_args()

hyperfine = "hyperfine --warmup 3 --shell=none".split(' ')

program_common = "--dir=data target/wasm32-wasip2/debug/wasi-compressor.wasm data/uncompressed-10M"

# With recording enabled
subprocess.run(hyperfine + [' '.join([args.rr, "-R path=test.trace", program_common])])
## With recording disabled
subprocess.run(hyperfine + [' '.join([args.rr, program_common])])
## On stock upstream
subprocess.run(hyperfine + [' '.join([args.upstream, program_common])])