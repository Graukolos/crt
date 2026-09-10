"""Assemble results/index.json from a bench.sh manifest.

Reads the tab-separated manifest bench.sh appends to, collects machine and
toolchain metadata from the running system, and merges the result into any
index.json already present so that sweeps from several machines accumulate in
one file.
"""

import datetime
import json
import os
import re
import subprocess
import sys

MACHINE = os.environ["MACHINE"]
OUT = os.environ["OUT"]
MANIFEST = os.environ["MANIFEST"]
ROOT = os.environ["ROOT"]
RUNS = int(os.environ["RUNS"])
WARMUP = int(os.environ["WARMUP"])
CAP = int(os.environ["CAP"])
FIRE_BUDGET = int(os.environ["FIRE_BUDGET"])

AXIS = {
    "baseline": "cores",
    "core-scaling": "cores",
    "cap-scaling": "cap",
    "budget-scaling": "fire_budget",
}

NETWORK_PATHS = {
    "balanced": ("custom-networks/balanced/cal", "custom-networks/balanced/xdf/gen.xdf"),
    "wide": ("custom-networks/wide/cal", "custom-networks/wide/xdf/gen.xdf"),
    "pipeline": ("custom-networks/pipeline/cal", "custom-networks/pipeline/xdf/gen.xdf"),
    "zigbee": (
        "custom-networks/ZigBee/src",
        "custom-networks/ZigBee/src/multitoken_tx/Top_ZigBee_tx.xdf",
    ),
}


def sh(*args, cwd=None):
    try:
        out = subprocess.run(
            args, cwd=cwd, capture_output=True, text=True, timeout=30, check=False
        )
    except (OSError, subprocess.SubprocessError):
        return "unknown"
    return out.stdout.strip() or "unknown"


def first_line(text):
    return text.splitlines()[0].strip() if text and text != "unknown" else "unknown"


def read_file(path, default=""):
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            return fh.read()
    except OSError:
        return default


def cpu_model():
    for line in read_file("/proc/cpuinfo").splitlines():
        if line.startswith(("model name", "Model")):
            return line.split(":", 1)[1].strip()
    return "unknown"


def physical_cores():
    ids = set()
    core, package = None, None
    for line in read_file("/proc/cpuinfo").splitlines():
        if line.startswith("core id"):
            core = line.split(":", 1)[1].strip()
        elif line.startswith("physical id"):
            package = line.split(":", 1)[1].strip()
        elif not line.strip() and core is not None:
            ids.add((package, core))
            core, package = None, None
    if core is not None:
        ids.add((package, core))
    return len(ids) or os.cpu_count()


def ram_gb():
    m = re.search(r"MemTotal:\s+(\d+) kB", read_file("/proc/meminfo"))
    return round(int(m.group(1)) / 1024 / 1024) if m else 0


def governor():
    g = read_file(
        "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor", "unknown"
    ).strip()
    return g or "unknown"


def machine_info():
    return {
        "cpu": cpu_model(),
        "cores": physical_cores(),
        "threads": os.cpu_count(),
        "ram_gb": ram_gb(),
        "kernel": sh("uname", "-r"),
        "governor": governor(),
    }


def toolchain_info():
    dcg = os.path.join(ROOT, "Dataflow_Code_Generator")
    return {
        "rustc": first_line(sh("rustc", "--version")).removeprefix("rustc "),
        "gcc": first_line(sh("gcc", "-dumpfullversion", "-dumpversion")),
        "hyperfine": first_line(sh("hyperfine", "--version")).removeprefix("hyperfine "),
        "crt": sh("git", "-C", ROOT, "rev-parse", "--short", "HEAD"),
        "dcg": sh("git", "-C", dcg, "rev-parse", "--short", "HEAD"),
    }


def network_info():
    info = {}
    for name, (cal_dir, xdf) in NETWORK_PATHS.items():
        cal_path = os.path.join(ROOT, cal_dir)
        actors = 0
        for dirpath, _, filenames in os.walk(cal_path):
            actors += sum(1 for f in filenames if f.endswith(".cal"))
        connections = read_file(os.path.join(ROOT, xdf)).count("<Connection")
        if actors or connections:
            info[name] = {"actors": actors, "connections": connections}
    return info


def params_for(experiment):
    p = {"runs": RUNS, "warmup": WARMUP}
    if experiment != "cap-scaling":
        p["cap"] = CAP
    if experiment != "budget-scaling":
        p["fire_budget"] = FIRE_BUDGET
    return p


def main():
    rows = []
    for line in read_file(MANIFEST).splitlines():
        if not line.strip():
            continue
        parts = line.split("\t")
        if len(parts) != 9:
            print(f"index.py: skipping malformed manifest line: {line}", file=sys.stderr)
            continue
        rows.append(parts)

    if not rows:
        print("index.py: manifest is empty, nothing to write", file=sys.stderr)
        return 1

    grouped = {}
    for experiment, network, config, cores, cap, budget, status, rel, result in rows:
        axis = AXIS[experiment]
        m = {"network": network, "config": config, "cores": int(cores)}
        if axis == "cap":
            m["cap"] = int(cap)
        elif axis == "fire_budget":
            m["fire_budget"] = int(budget)
        if status == "ok":
            m["file"] = rel
            m["result"] = int(result)
        else:
            m["status"] = status
        grouped.setdefault(experiment, []).append(m)

    path = os.path.join(OUT, "index.json")
    index = json.loads(read_file(path, "{}") or "{}")
    index.pop("fixture", None)
    index["schema"] = 1
    index.setdefault("configs", [])
    index.setdefault("machines", {})
    index.setdefault("runs", [])

    canonical = os.environ.get("CONFIGS", "").split()
    if not canonical:
        for row in rows:
            if row[2] not in canonical:
                canonical.append(row[2])
    index["configs"] = canonical

    index["networks"] = {**index.get("networks", {}), **network_info()}
    index["machines"][MACHINE] = machine_info()

    today = datetime.date.today().isoformat()
    toolchain = toolchain_info()
    fresh = {
        experiment: {
            "experiment": experiment,
            "machine": MACHINE,
            "date": today,
            "toolchain": toolchain,
            "params": params_for(experiment),
            "axis": AXIS[experiment],
            "measurements": measurements,
        }
        for experiment, measurements in grouped.items()
    }

    index["runs"] = [
        r
        for r in index["runs"]
        if not (r["machine"] == MACHINE and r["experiment"] in fresh)
    ] + list(fresh.values())
    index["runs"].sort(key=lambda r: (r["machine"], r["experiment"]))

    os.makedirs(OUT, exist_ok=True)
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(index, fh, indent=2)
        fh.write("\n")

    for experiment, measurements in sorted(grouped.items()):
        failed = sum(1 for m in measurements if "status" in m)
        print(f"    {experiment}: {len(measurements)} measurements, {failed} failed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
