#!/usr/bin/env python3
"""Check overrules still reads the schema Overture releases use.

1. Reads Overture's release calendar and finds the newest release whose schema version is
   known (past or announced).
2. Compares the values overrules knows (`cargo run --example supported_values`) with that
   schema version's enums, read from the OvertureMaps/schema repository at its tag.
3. Warns when a release in a "major change" month is coming up.

Fails if the schema version moved on, or if any list differs. Standard library only.
"""

import datetime as dt
import json
import os
import re
import subprocess
import sys
import urllib.request

CALENDAR = "https://docs.overturemaps.org/release-calendar.json"
RAW = "https://raw.githubusercontent.com/OvertureMaps/schema/v{version}/packages/{path}"
COMMON = "overture-schema-common/src/overture/schema/common"
SEGMENT = "overture-schema-theme-transportation/src/overture/schema/transportation/segment"

# Our list name → (file in the schema repo, enum class).
SOURCES = {
    "heading": (f"{COMMON}/scoping/heading.py", "Heading"),
    "travel_mode": (f"{COMMON}/scoping/travel_mode.py", "TravelMode"),
    "vehicle_dimension": (f"{COMMON}/scoping/vehicle.py", "VehicleDimension"),
    "comparison": (f"{COMMON}/scoping/vehicle.py", "VehicleRelation"),
    "length_unit": (f"{COMMON}/unit.py", "LengthUnit"),
    "weight_unit": (f"{COMMON}/unit.py", "WeightUnit"),
    "speed_unit": (f"{COMMON}/unit.py", "SpeedUnit"),
    "access_type": (f"{SEGMENT}/_common.py", "AccessType"),
}

# How far ahead to warn about a release in a major change month.
MAJOR_CHANGE_WARNING = dt.timedelta(days=60)


def fetch(url):
    with urllib.request.urlopen(url, timeout=30) as r:
        return r.read().decode()


def enum_values(source, cls):
    """Values of `class <cls>(...)` in a Python file: `NAME = "v"` or `NAME = ("v", "doc")`."""
    m = re.search(rf"^class {cls}\(.*?(?=^class |\Z)", source, re.S | re.M)
    if not m:
        raise SystemExit(f"enum {cls} not found; the schema layout may have changed")
    return re.findall(r'^\s+[A-Z][A-Z0-9_]* = \(?\s*"([^"]+)"', m.group(0), re.M)


def report(level, message):
    print(f"::{level}::{message}" if os.environ.get("GITHUB_ACTIONS") else f"{level}: {message}")


def main():
    ours = json.loads(
        subprocess.run(
            ["cargo", "run", "-q", "--example", "supported_values"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    supported = ours["schema_version"]

    releases = json.loads(fetch(CALENDAR))["releases"]
    known = [r for r in releases if r.get("schemaVersion")]
    newest = max(known, key=lambda r: r["date"])
    target = newest["schemaVersion"].removeprefix("v")
    print(f"overrules reads schema {supported}; newest release with a known schema: "
          f"{newest['dataVersion']} ({newest['date']}) uses {target}")

    failed = False
    if target != supported:
        report("error", f"release {newest['dataVersion']} on {newest['date']} uses schema {target}, "
                        f"but overrules reads {supported}")
        failed = True

    files = {}
    for name, (path, cls) in SOURCES.items():
        if path not in files:
            files[path] = fetch(RAW.format(version=target, path=path))
        theirs = enum_values(files[path], cls)
        mine = ours["lists"][name]
        new, gone = sorted(set(theirs) - set(mine)), sorted(set(mine) - set(theirs))
        if new or gone:
            failed = True
            report("error", f"{name}: schema {target} adds {new or 'nothing'}, drops {gone or 'nothing'}")
        else:
            print(f"  {name:<18} matches ({len(mine)} values)")

    today = dt.date.today()
    for r in releases:
        date = dt.date.fromisoformat(r["date"])
        if r.get("majorChangeMonth") and today <= date <= today + MAJOR_CHANGE_WARNING:
            report("warning", f"release {r['dataVersion']} on {r['date']} is in a major change month; "
                              f"its schema version is {r.get('schemaVersion') or 'not announced yet'}")

    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
