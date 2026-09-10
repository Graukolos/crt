"""Print the tables that live in results/index.json.

usage: python3 experiments/report.py [results_dir] [machine]
"""

import json
import os
import sys


def load(results_dir):
    with open(os.path.join(results_dir, "index.json"), encoding="utf-8") as fh:
        return json.load(fh)


def value(results_dir, m):
    if m.get("status", "ok") != "ok":
        return None
    with open(os.path.join(results_dir, m["file"]), encoding="utf-8") as fh:
        r = json.load(fh)["results"][m.get("result", 0)]
    return r["mean"] * 1000, (r["stddev"] or 0) * 1000


def table(title, columns, rows, cell):
    print(f"\n    {title}\n")
    print("    " + "config".ljust(18) + "".join(str(c).rjust(14) for c in columns))
    for row in rows:
        print("    " + row.ljust(18) + "".join(cell(row, c).rjust(14) for c in columns))


def main():
    results_dir = sys.argv[1] if len(sys.argv) > 1 else "results"
    only = sys.argv[2] if len(sys.argv) > 2 else None
    index = load(results_dir)

    for run in index["runs"]:
        if only and run["machine"] != only:
            continue
        axis = run["axis"]
        data = {}
        for m in run["measurements"]:
            data[(m["network"], m["config"], m.get(axis))] = value(results_dir, m)

        networks = sorted({k[0] for k in data})
        configs = [c for c in index["configs"] if any(k[1] == c for k in data)]

        for network in networks:
            columns = sorted({k[2] for k in data if k[0] == network})

            def wall(config, col, network=network):
                if (network, config, col) not in data:
                    return "-"
                v = data[(network, config, col)]
                return "FAIL" if v is None else f"{v[0]:.0f} ({v[1]:.0f})"

            head = f"{run['machine']}/{run['experiment']}/{network}"
            table(f"{head}: mean wall time, ms (stddev), by {axis}", columns, configs, wall)

            if axis == "cores" and len(columns) > 1:
                base = columns[0]

                def speedup(config, col, network=network, base=base):
                    v = data.get((network, config, col))
                    b = data.get((network, config, base))
                    return f"{b[0] / v[0]:.2f}x" if v and b else "-"

                table(f"{head}: speedup vs own {base}-core time", columns, configs, speedup)
    return 0


if __name__ == "__main__":
    sys.exit(main())
