#!/usr/bin/env python3
import argparse
import subprocess
import json
import sys
import os
import statistics
from collections import defaultdict

# Try importing matplotlib
try:
    import matplotlib.pyplot as plt
except ImportError:
    plt = None

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

def plot_results(all_results, n_values, d_values):
    if not plt:
        print("Matplotlib not found, skipping plots.")
        return

    # Create plots directory if it doesn't exist
    os.makedirs("plots", exist_ok=True)

    strategies = ["Uffd", "SoftDirty", "EmulatedSoftDirty"]
    
    # Plot 1: Fixed D, X=N, Y=Time
    for d in d_values:
        fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(14, 6))
        fig.suptitle(f"Benchmark Results (d={d})")
        
        # Plot Scan Time
        for strat in strategies:
            x = []
            y = []
            yerr = []
            for n in sorted(n_values):
                res = all_results.get((n, d), {}).get(strat)
                if res:
                    x.append(n)
                    y.append(res["scan_avg"])
                    yerr.append(res["scan_stdev"])
            ax1.errorbar(x, y, yerr=yerr, label=strat, marker='o', capsize=5)
        
        ax1.set_title("Scan Time vs N")
        ax1.set_xlabel("N (ops)")
        ax1.set_xscale('log', base=10)
        ax1.set_ylabel("Time (µs)")
        ax1.legend()
        ax1.grid(True, alpha=0.3)

        # Plot Harness Time
        for strat in strategies:
            x = []
            y = []
            yerr = []
            for n in sorted(n_values):
                res = all_results.get((n, d), {}).get(strat)
                if res:
                    x.append(n)
                    y.append(res["harness_avg"])
                    yerr.append(res["harness_stdev"])
            ax2.errorbar(x, y, yerr=yerr, label=strat, marker='o', capsize=5)
            
        ax2.set_title("Harness Time vs N")
        ax2.set_xlabel("N (ops)")
        ax2.set_xscale('log', base=10)
        ax2.set_ylabel("Time (µs)")
        ax2.legend()
        ax2.grid(True, alpha=0.3)
        
        plt.tight_layout()
        filename = f"plots/benchmark_d_{d}.png"
        plt.savefig(filename)
        print(f"Saved plot to {filename}")

    # Plot 2: Fixed N, X=D, Y=Time
    for n in n_values:
        fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(14, 6))
        fig.suptitle(f"Benchmark Results (n={n})")
        
        # Plot Scan Time
        for strat in strategies:
            x = []
            y = []
            yerr = []
            for d in sorted(d_values):
                res = all_results.get((n, d), {}).get(strat)
                if res:
                    x.append(d)
                    y.append(res["scan_avg"])
                    yerr.append(res["scan_stdev"])
            ax1.errorbar(x, y, yerr=yerr, label=strat, marker='o', capsize=5)
        
        ax1.set_title("Scan Time vs D")
        ax1.set_xlabel("D (stddev)")
        ax1.set_xscale('log', base=2)
        ax1.set_ylabel("Time (µs)")
        ax1.legend()
        ax1.grid(True, alpha=0.3)

        # Plot Harness Time
        for strat in strategies:
            x = []
            y = []
            yerr = []
            for d in sorted(d_values):
                res = all_results.get((n, d), {}).get(strat)
                if res:
                    x.append(d)
                    y.append(res["harness_avg"])
                    yerr.append(res["harness_stdev"])
            ax2.errorbar(x, y, yerr=yerr, label=strat, marker='o', capsize=5)
            
        ax2.set_title("Harness Time vs D")
        ax2.set_xlabel("D (stddev)")
        ax2.set_xscale('log', base=2)
        ax2.set_ylabel("Time (µs)")
        ax2.legend()
        ax2.grid(True, alpha=0.3)
        
        plt.tight_layout()
        filename = f"plots/benchmark_n_{n}.png"
        plt.savefig(filename)
        print(f"Saved plot to {filename}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--n", "-n", nargs="+", type=int, required=True, help="List of N values")
    parser.add_argument("--d", "-d", nargs="+", type=float, required=True, help="List of D values")
    parser.add_argument("--runs", "-r", type=int, default=20, help="Number of runs")
    args = parser.parse_args()

    # Build
    print("Building snapshot binary...")
    subprocess.check_call(["cargo", "build", "--release", "--bin", "snapshot"])
    binary_path = "../target/release/snapshot"

    all_results = {}

    for n in args.n:
        for d in args.d:
            try:
                res = run_benchmark(n, d, args.runs, binary_path)
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

    plot_results(all_results, args.n, args.d)

if __name__ == "__main__":
    main()
