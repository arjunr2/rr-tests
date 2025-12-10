#!/usr/bin/env python3
import argparse
import json
import sys
import os
import matplotlib.pyplot as plt
import matplotlib.colors as mcolors

def plot_results(data, output_dir):
    # Reconstruct all_results map
    all_results = {}
    n_values = set()
    d_values = set()
    
    for entry in data:
        n = entry["n"]
        d = entry["d"]
        n_values.add(n)
        d_values.add(d)
        all_results[(n, d)] = entry["results"]

    strategies = ["Uffd", "SoftDirty", "EmulatedSoftDirty"]
    
    # Plot 1: Fixed D, X=N, Y=Time
    for d in sorted(d_values):
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
        filename = os.path.join(output_dir, f"benchmark_d_{d}.png")
        plt.savefig(filename)
        print(f"Saved plot to {filename}")
        plt.close(fig)

    # Plot 2: Fixed N, X=D, Y=Time
    for n in sorted(n_values):
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
        filename = os.path.join(output_dir, f"benchmark_n_{n}.png")
        plt.savefig(filename)
        print(f"Saved plot to {filename}")
        plt.close(fig)

def plot_heatmaps(data, output_dir):
    # Reconstruct all_results map
    all_results = {}
    n_values = set()
    d_values = set()
    
    for entry in data:
        n = entry["n"]
        d = entry["d"]
        n_values.add(n)
        d_values.add(d)
        all_results[(n, d)] = entry["results"]

    n_sorted = sorted(n_values)
    d_sorted = sorted(d_values)
    
    # Prepare grids
    # Rows = D, Cols = N
    rows = len(d_sorted)
    cols = len(n_sorted)
    
    # Speedup = Baseline / Target
    
    # 1. Scan: Uffd vs SoftDirty (Baseline = SoftDirty)
    scan_speedup_uffd_vs_sd = [[0.0] * cols for _ in range(rows)]
    
    # 2. Harness: Uffd vs SoftDirty (Baseline = SoftDirty)
    harness_speedup_uffd_vs_sd = [[0.0] * cols for _ in range(rows)]
    
    # 3. Harness: Uffd vs ESD (Baseline = ESD)
    harness_speedup_uffd_vs_esd = [[0.0] * cols for _ in range(rows)]
    
    # 4. Harness: SoftDirty vs ESD (Baseline = ESD)
    harness_speedup_sd_vs_esd = [[0.0] * cols for _ in range(rows)]
    
    for r, d in enumerate(d_sorted):
        for c, n in enumerate(n_sorted):
            res = all_results.get((n, d))
            if not res:
                continue
            
            esd = res.get("EmulatedSoftDirty")
            uffd = res.get("Uffd")
            sd = res.get("SoftDirty")
            
            # Uffd vs SoftDirty
            if uffd and sd:
                if uffd["scan_avg"] > 0:
                    scan_speedup_uffd_vs_sd[r][c] = sd["scan_avg"] / uffd["scan_avg"]
                if uffd["harness_avg"] > 0:
                    harness_speedup_uffd_vs_sd[r][c] = sd["harness_avg"] / uffd["harness_avg"]

            # Uffd vs ESD
            if esd and uffd:
                if uffd["harness_avg"] > 0:
                    harness_speedup_uffd_vs_esd[r][c] = esd["harness_avg"] / uffd["harness_avg"]
            
            # SoftDirty vs ESD
            if esd and sd:
                if sd["harness_avg"] > 0:
                    harness_speedup_sd_vs_esd[r][c] = esd["harness_avg"] / sd["harness_avg"]

    # Calculate global max for unified scale
    all_values = []
    for row in scan_speedup_uffd_vs_sd: all_values.extend(row)
    for row in harness_speedup_uffd_vs_sd: all_values.extend(row)
    for row in harness_speedup_uffd_vs_esd: all_values.extend(row)
    for row in harness_speedup_sd_vs_esd: all_values.extend(row)
    
    global_max = max(all_values) if all_values else 1.01
    if global_max < 1.01: global_max = 1.01

    # Plotting
    fig, axes = plt.subplots(2, 2, figsize=(16, 12))
    fig.suptitle("Speedup Heatmaps (Green > 1.0x, Red < 1.0x)")
    
    heatmaps = [
        (axes[0, 0], scan_speedup_uffd_vs_sd, "Scan (Uffd over SoftDirty)"),
        (axes[0, 1], harness_speedup_uffd_vs_sd, "Harness (Uffd over SoftDirty)"),
        (axes[1, 0], harness_speedup_uffd_vs_esd, "Harness (Uffd over ESD)"),
        (axes[1, 1], harness_speedup_sd_vs_esd, "Harness (SoftDirty over ESD)"),
    ]
    
    last_im = None
    for ax, data, title in heatmaps:
        norm = mcolors.TwoSlopeNorm(vmin=0, vcenter=1, vmax=global_max)
        
        im = ax.imshow(data, origin='lower', cmap='RdYlGn', aspect='auto', norm=norm)
        last_im = im
        ax.set_title(title)
        ax.set_xlabel("N (ops)")
        ax.set_ylabel("D (stddev)")
        
        # Set ticks
        ax.set_xticks(range(cols))
        ax.set_xticklabels(n_sorted)
        ax.set_yticks(range(rows))
        ax.set_yticklabels(d_sorted)
        
        # Add text annotations
        for i in range(rows):
            for j in range(cols):
                val = data[i][j]
                text = ax.text(j, i, f"{val:.2f}x",
                               ha="center", va="center", color="black", fontsize=9)
        
    plt.tight_layout()
    
    if last_im:
        fig.subplots_adjust(right=0.85)
        cbar_ax = fig.add_axes([0.88, 0.15, 0.02, 0.7])
        fig.colorbar(last_im, cax=cbar_ax, label="Speedup Factor")

    filename = os.path.join(output_dir, "speedup_heatmap.png")
    plt.savefig(filename)
    print(f"Saved heatmap to {filename}")
    plt.close(fig)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("input_file", help="Input JSON file with benchmark results")
    parser.add_argument("--output-dir", "-o", default="plots", help="Directory to save plots")
    args = parser.parse_args()

    if not os.path.exists(args.input_file):
        print(f"Error: Input file {args.input_file} not found.", file=sys.stderr)
        sys.exit(1)

    with open(args.input_file, 'r') as f:
        data = json.load(f)

    os.makedirs(args.output_dir, exist_ok=True)

    # Only plot heatmaps
    plot_heatmaps(data, args.output_dir)

if __name__ == "__main__":
    main()
