#!/usr/bin/env python3
"""Rebuild the small Parquet reader fixtures. Requires the DuckDB CLI."""

import json
from pathlib import Path
import struct
import subprocess
import tempfile


def main():
    fixtures = Path(__file__).resolve().parents[1] / "tests" / "fixtures"
    features = json.loads((fixtures / "nyc.geojson").read_text())["features"]
    rows = []
    for feature in features:
        coords = feature["geometry"]["coordinates"]
        wkb = struct.pack("<BII", 1, 2, len(coords))
        wkb += b"".join(struct.pack("<dd", *c[:2]) for c in coords)
        rows.append({**feature["properties"], "wkb_hex": wkb.hex()})

    with tempfile.TemporaryDirectory() as directory:
        source = Path(directory) / "rows.jsonl"
        source.write_text("".join(json.dumps(row) + "\n" for row in rows))
        table = f"read_json_auto('{source}')"
        schema = json.loads(subprocess.check_output(
            ["duckdb", "-json", "-c", f"DESCRIBE SELECT * FROM {table}"], text=True
        ))
        # DuckDB infers UUIDs from GERS strings. Overture release files use strings,
        # including in nested connector and transition references.
        columns = ", ".join(
            f'CAST("{c["column_name"]}" AS {c["column_type"].replace("UUID", "VARCHAR")}) '
            f'AS "{c["column_name"]}"'
            for c in schema if c["column_name"] != "wkb_hex"
        )
        for name, codec in [("nyc-snappy", "SNAPPY"), ("nyc-zstd", "ZSTD"),
                            ("invalid-record", "ZSTD"), ("invalid-wkb", "ZSTD")]:
            sample = json.loads(json.dumps(rows))
            if name == "invalid-record":
                sample[2]["access_restrictions"][0]["access_type"] = "permissive"
            elif name == "invalid-wkb":
                sample[1]["wkb_hex"] = "00"
            source.write_text("".join(json.dumps(row) + "\n" for row in sample))
            target = fixtures / f"{name}.parquet"
            sql = (f"COPY (SELECT {columns}, from_hex(wkb_hex) AS geometry FROM {table}) "
                   f"TO '{target}' (FORMAT PARQUET, COMPRESSION {codec})")
            subprocess.run(["duckdb", "-c", sql], check=True)
            print(target)


if __name__ == "__main__":
    main()
