#!/usr/bin/env python3
import argparse
import subprocess
import json
import sys
import os
import statistics

def extract_page_num(val):
    if isinstance(val, list):
        return val[0]
    return val

def get_dirty_pages(scan_result):
    pages = set()
    for region in scan_result.get("regions", []):
        start = extract_page_num(region["start"])
        end = extract_page_num(region["end"])
        for p in range(start, end):
            pages.add(p)
    return pages

def validate_run(run_idx, uffd_res, emulated_res, soft_dirty_res):
    # 1. Uffd and EmulatedSoftDirty must be identical
    if uffd_res and emulated_res:
        if uffd_res["scan"] != emulated_res["scan"]:
            print(f"Error: Scan mismatch at run index {run_idx} between Uffd and EmulatedSoftDirty", file=sys.stderr)
            return False

    # 2. SoftDirty must be a superset of Uffd/Emulated
    reference_res = uffd_res or emulated_res
    if soft_dirty_res and reference_res:
        sd_pages = get_dirty_pages(soft_dirty_res["scan"])
        ref_pages = get_dirty_pages(reference_res["scan"])
        
        if not sd_pages.issuperset(ref_pages):
            missing = ref_pages - sd_pages
            print(f"Error: SoftDirty is NOT a superset of reference at run index {run_idx}", file=sys.stderr)
            print(f"  Missing pages: {sorted(list(missing))}", file=sys.stderr)
            return False
    return True

def parse_duration_micros(duration_obj):
    if not duration_obj:
        return 0.0
    secs = duration_obj.get("secs", 0)
    nanos = duration_obj.get("nanos", 0)
    return (secs * 1_000_000) + (nanos / 1_000.0)

def run_benchmark(n, d, r, binary_path):
    strategies = ["uffd", "soft-dirty", "emulated-soft-dirty"]
    files = {}
    
    print(f"Running benchmark for n={n}, d={d}...")

    for strat in strategies:
        outfile = f"{strat}_{n}_{d}.json"
        cmd = [
            binary_path,
            "-d", str(d),
            "-n", str(n),
            "-r", str(r),
            strat,
            "-o", outfile
        ]
        # Suppress output unless error
        subprocess.check_call(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        files[strat] = outfile

    # Load and Validate
    data_map = {}
    for strat, fname in files.items():
        with open(fname, 'r') as f:
            data_map[strat] = json.load(f)
        os.remove(fname) # Cleanup

    # Normalize keys
    json_map = {}
    for strat, data in data_map.items():
        json_map[data["strategy"]] = data["results"]

    # Validate
    for i in range(r):
        uffd = json_map.get("Uffd", [None]*r)[i]
        esd = json_map.get("EmulatedSoftDirty", [None]*r)[i]
        sd = json_map.get("SoftDirty", [None]*r)[i]
        
        if not validate_run(i, uffd, esd, sd):
            raise RuntimeError(f"Validation failed for n={n}, d={d}, run={i}")

    # Calculate averages
    results = {}
    for strat, res_list in json_map.items():
        scan_times = [parse_duration_micros(r["scan_duration"]) for r in res_list]
        harness_times = [parse_duration_micros(r["harness_duration"]) for r in res_list]
        results[strat] = {
            "scan_avg": statistics.mean(scan_times),
            "scan_stdev": statistics.stdev(scan_times) if len(scan_times) > 1 else 0,
            "harness_avg": statistics.mean(harness_times),
            "harness_stdev": statistics.stdev(harness_times) if len(harness_times) > 1 else 0,
        }
    
    return results

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--n", "-n", nargs="+", type=int, required=True, help="List of N values")
    parser.add_argument("--d", "-d", nargs="+", type=float, required=True, help="List of D values")
    parser.add_argument("--runs", "-r", type=int, default=20, help="Number of runs")
    parser.add_argument("--output", "-o", type=str, default="benchmark_results.json", help="Output JSON file")
    args = parser.parse_args()

    # Build
    print("Building snapshot binary...")
    subprocess.check_call(["cargo", "build", "--release", "--bin", "snapshot"])
    binary_path = "../target/release/snapshot"

    all_results = {}
    # Structure for JSON output: list of objects with n, d, results
    output_data = []

    for n in args.n:
        for d in args.d:
            try:
                res = run_benchmark(n, d, args.runs, binary_path)
                # Convert tuple key to string for JSON compatibility if needed, 
                # but better to structure as list of objects
                entry = {
                    "n": n,
                    "d": d,
                    "results": res
                }
                output_data.append(entry)
                all_results[(n, d)] = res
            except Exception as e:
                print(f"Benchmark failed for n={n}, d={d}: {e}")
                sys.exit(1)

    # Print Summary Table
    print("\nSummary Results (Avg µs):")
    print(f"{'N':<10} | {'D':<10} | {'Strategy':<20} | {'Scan':<10} | {'Harness':<10}")
    print("-" * 70)
    for (n, d), res_map in all_results.items():
        for strat, metrics in res_map.items():
            print(f"{n:<10} | {d:<10} | {strat:<20} | {metrics['scan_avg']:<10.2f} | {metrics['harness_avg']:<10.2f}")

    # Save to JSON
    with open(args.output, 'w') as f:
        json.dump(output_data, f, indent=2)
    print(f"\nResults saved to {args.output}")

if __name__ == "__main__":
    main()
