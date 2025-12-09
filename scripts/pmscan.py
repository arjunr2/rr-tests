import argparse
import json
import sys

def parse_duration_micros(duration_obj):
    secs = duration_obj.get("secs", 0)
    nanos = duration_obj.get("nanos", 0)
    return (secs * 1_000_000) + (nanos / 1_000.0)

def main():
    parser = argparse.ArgumentParser(description="Compare soft-dirty and uffd benchmark results.")
    parser.add_argument("soft_dirty_path", help="Path to the soft-dirty JSON output file")
    parser.add_argument("uffd_path", help="Path to the uffd JSON output file")
    
    args = parser.parse_args()
    
    try:
        with open(args.soft_dirty_path, 'r') as f:
            soft_dirty_data = json.load(f)
    except Exception as e:
        print(f"Error reading soft-dirty file: {e}", file=sys.stderr)
        sys.exit(1)

    try:
        with open(args.uffd_path, 'r') as f:
            uffd_data = json.load(f)
    except Exception as e:
        print(f"Error reading uffd file: {e}", file=sys.stderr)
        sys.exit(1)

    if len(soft_dirty_data) != len(uffd_data):
        print(f"Error: Files have different number of runs. Soft-dirty: {len(soft_dirty_data)}, UFFD: {len(uffd_data)}", file=sys.stderr)
        sys.exit(1)

    print(f"Number of runs to compare: {len(soft_dirty_data)}")
    soft_dirty_durations = []
    uffd_durations = []

    for i, (sd_run, uffd_run) in enumerate(zip(soft_dirty_data, uffd_data)):
        # Validate scan fields are identical
        if sd_run["scan"] != uffd_run["scan"]:
            print(f"Error: Scan mismatch at run index {i}", file=sys.stderr)
            print(f"Soft-dirty scan: {json.dumps(sd_run['scan'], indent=2)}", file=sys.stderr)
            print(f"UFFD scan: {json.dumps(uffd_run['scan'], indent=2)}", file=sys.stderr)
            sys.exit(1)
        
        soft_dirty_durations.append(parse_duration_micros(sd_run["duration"]))
        uffd_durations.append(parse_duration_micros(uffd_run["duration"]))

    avg_sd = sum(soft_dirty_durations) / len(soft_dirty_durations) if soft_dirty_durations else 0
    avg_uffd = sum(uffd_durations) / len(uffd_durations) if uffd_durations else 0

    print("Validation Successful: All scan fields match.")
    print(f"Average Soft-Dirty Duration: {avg_sd:.2f} µs")
    print(f"Average UFFD Duration:       {avg_uffd:.2f} µs")

if __name__ == "__main__":
    main()
