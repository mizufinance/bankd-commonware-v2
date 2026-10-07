"""Keep the RUSTSEC-2025-0055 exception limited to inactive log formatting."""

import json
import sys


def vulnerable_formatters(metadata):
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    affected = []
    for package in metadata["packages"]:
        if package["name"] != "tracing-subscriber":
            continue
        version = tuple(int(part) for part in package["version"].split("-")[0].split("."))
        if version < (0, 3, 20) and "fmt" in nodes[package["id"]]["features"]:
            affected.append(package["version"])
    return affected


if __name__ == "__main__":
    with open(sys.argv[1]) as source:
        affected = vulnerable_formatters(json.load(source))
    for version in affected:
        print(
            f"::error::tracing-subscriber {version} enables vulnerable formatting; "
            "RUSTSEC-2025-0055 can no longer be excepted. Upgrade to >=0.3.20."
        )
    sys.exit(bool(affected))
