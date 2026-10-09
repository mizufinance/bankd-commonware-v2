"""Fail a CI summary when a required job fails, is cancelled, or is skipped."""

import json
import os
import sys


def failures(needs, allowed_skips):
    return [
        name
        for name, job in needs.items()
        if job["result"] != "success"
        and not (job["result"] == "skipped" and name in allowed_skips)
    ]


if __name__ == "__main__":
    needs = json.loads(os.environ["NEEDS"])
    failed = failures(needs, set(os.environ.get("ALLOWED_SKIPS", "").split(",")))
    for name in failed:
        print(f"::error::{name}: {needs[name]['result']}")
    sys.exit(bool(failed))
