import argparse
import json
import sys

def parse_duration_micros(duration_obj):
    if not duration_obj:
        return 0.0
    secs = duration_obj.get("secs", 0)
    nanos = duration_obj.get("nanos", 0)
    return (secs * 1_000_000) + (nanos / 1_000.0)

def main():
    parser = argparse.ArgumentParser(description="Compare benchmark results from multiple JSON files.")
    parser.add_argument("files", nargs='+', help="Paths to the JSON output files")
    
    args = parser.parse_args()
    
    data_files = []
    for file_path in args.files:
        try:
            with open(file_path, 'r') as f:
                data = json.load(f)
                # Check if it matches the expected structure
                if isinstance(data, dict) and "strategy" in data and "results" in data:
                    data_files.append(data)
                else:
                    print(f"Error: File {file_path} does not match expected format (object with 'strategy' and 'results').", file=sys.stderr)
                    sys.exit(1)
        except Exception as e:
            print(f"Error reading file {file_path}: {e}", file=sys.stderr)
            sys.exit(1)

    if not data_files:
        print("No data loaded.", file=sys.stderr)
        sys.exit(1)

    # Validate number of runs
    first_file = data_files[0]
    num_runs = len(first_file["results"])
    
    for data in data_files[1:]:
        if len(data["results"]) != num_runs:
            print(f"Error: Files have different number of runs. {first_file['strategy']}: {num_runs}, {data['strategy']}: {len(data['results'])}", file=sys.stderr)
            sys.exit(1)

    print(f"Number of runs to compare: {num_runs}")

    # Validate scans match across all files for each run
    # We compare everything against the first file's results
    for i in range(num_runs):
        base_scan = first_file["results"][i]["scan"]
        for data in data_files[1:]:
            current_scan = data["results"][i]["scan"]
            if base_scan != current_scan:
                print(f"Error: Scan mismatch at run index {i} between {first_file['strategy']} and {data['strategy']}", file=sys.stderr)
                
                if base_scan.get("walk_start") != current_scan.get("walk_start"):
                     print(f"  walk_start mismatch: {base_scan.get('walk_start')} vs {current_scan.get('walk_start')}", file=sys.stderr)
                if base_scan.get("walk_end") != current_scan.get("walk_end"):
                     print(f"  walk_end mismatch: {base_scan.get('walk_end')} vs {current_scan.get('walk_end')}", file=sys.stderr)

                base_regions = base_scan.get("regions", [])
                curr_regions = current_scan.get("regions", [])
                
                if len(base_regions) != len(curr_regions):
                    print(f"  Region count mismatch: {len(base_regions)} vs {len(curr_regions)}", file=sys.stderr)
                
                limit = min(len(base_regions), len(curr_regions))
                for r_idx in range(limit):
                    br = base_regions[r_idx]
                    cr = curr_regions[r_idx]
                    if br != cr:
                        print(f"  Mismatch at region index {r_idx}:", file=sys.stderr)
                        print(f"    {first_file['strategy']}: {json.dumps(br)}", file=sys.stderr)
                        print(f"    {data['strategy']}: {json.dumps(cr)}", file=sys.stderr)
                        break
                else:
                    # If loop completed without break, and lengths differ
                    if len(base_regions) > limit:
                         print(f"  Mismatch at region index {limit} (extra in base):", file=sys.stderr)
                         print(f"    {first_file['strategy']}: {json.dumps(base_regions[limit])}", file=sys.stderr)
                    elif len(curr_regions) > limit:
                         print(f"  Mismatch at region index {limit} (extra in current):", file=sys.stderr)
                         print(f"    {data['strategy']}: {json.dumps(curr_regions[limit])}", file=sys.stderr)
                
                sys.exit(1)

    print("Validation Successful: All scan fields match across all files.")
    print("-" * 90)
    print(f"{'Strategy':<30} | {'Avg Scan (µs)':<20} | {'Avg Harness (µs)':<20}")
    print("-" * 90)

    for data in data_files:
        strategy = data["strategy"]
        results = data["results"]
        
        scan_durations = [parse_duration_micros(r.get("scan_duration")) for r in results]
        harness_durations = [parse_duration_micros(r.get("harness_duration")) for r in results]
        
        avg_scan = sum(scan_durations) / len(scan_durations) if scan_durations else 0
        avg_harness = sum(harness_durations) / len(harness_durations) if harness_durations else 0
        
        print(f"{strategy:<30} | {avg_scan:<20.2f} | {avg_harness:<20.2f}")

if __name__ == "__main__":
    main()
