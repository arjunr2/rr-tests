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

    # Organize data by strategy
    strategies = {}
    for data in data_files:
        strategies[data["strategy"]] = data["results"]

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

    # Validate scans
    for i in range(num_runs):
        uffd_res = strategies.get("Uffd", [None] * num_runs)[i]
        emulated_res = strategies.get("EmulatedSoftDirty", [None] * num_runs)[i]
        soft_dirty_res = strategies.get("SoftDirty", [None] * num_runs)[i]

        # 1. Uffd and EmulatedSoftDirty must be identical
        if uffd_res and emulated_res:
            if uffd_res["scan"] != emulated_res["scan"]:
                print(f"Error: Scan mismatch at run index {i} between Uffd and EmulatedSoftDirty", file=sys.stderr)
                # Detailed diff could go here, but for now just fail
                # Reuse the logic from before if needed, or just print json
                print(f"  Uffd: {json.dumps(uffd_res['scan'])}", file=sys.stderr)
                print(f"  Emulated: {json.dumps(emulated_res['scan'])}", file=sys.stderr)
                sys.exit(1)

        # 2. SoftDirty must be a superset of Uffd/Emulated
        reference_res = uffd_res or emulated_res
        if soft_dirty_res and reference_res:
            sd_pages = get_dirty_pages(soft_dirty_res["scan"])
            ref_pages = get_dirty_pages(reference_res["scan"])
            
            if not sd_pages.issuperset(ref_pages):
                missing = ref_pages - sd_pages
                print(f"Error: SoftDirty is NOT a superset of {uffd_res and 'Uffd' or 'EmulatedSoftDirty'} at run index {i}", file=sys.stderr)
                print(f"  Missing pages in SoftDirty: {sorted(list(missing))}", file=sys.stderr)
                sys.exit(1)

    print("Validation Successful.")
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
