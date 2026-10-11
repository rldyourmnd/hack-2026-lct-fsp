#!/usr/bin/env python3

import argparse
import json
from pathlib import Path


REQUIRED_FIELDS = (
    "id",
    "algorithmVersion",
    "change",
    "expectedEffect",
    "failureCondition",
    "verification",
)


def check(root: Path) -> None:
    experiment_dir = root / "backend" / "experiments"
    records = sorted(experiment_dir.glob("*.json"))
    if not records:
        raise SystemExit("no experiment records found")

    errors: list[str] = []
    for path in records:
        relative = path.relative_to(root).as_posix()
        try:
            record = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            errors.append(f"{relative}: {error}")
            continue

        missing = [field for field in REQUIRED_FIELDS if not record.get(field)]
        if missing:
            errors.append(f"{relative}: missing {', '.join(missing)}")
        if record.get("id") != path.stem:
            errors.append(f"{relative}: id must match the file name")

        verification = record.get("verification")
        if not isinstance(verification, dict):
            errors.append(f"{relative}: verification must be an object")
            continue
        if not verification.get("command"):
            errors.append(f"{relative}: verification.command is required")
        if verification.get("artifact") != relative:
            errors.append(f"{relative}: verification.artifact must point to this record")

    if errors:
        raise SystemExit("\n".join(errors))
    print(f"checked {len(records)} experiment records")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("check",))
    args = parser.parse_args()
    if args.command == "check":
        check(Path(__file__).resolve().parents[1])


if __name__ == "__main__":
    main()
