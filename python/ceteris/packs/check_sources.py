"""Validate the data-source licence manifest (Sections 2.5, 16, 22).

Standard library only, so CI can run it before the Python package exists:

    python3 -I python/ceteris/packs/check_sources.py
"""

import datetime
import sys
import tomllib
from pathlib import Path

MANIFEST = Path(__file__).with_name("sources.toml")

REQUIRED = {
    "id": str,
    "name": str,
    "provider": str,
    "version": str,
    "role": str,
    "licence": str,
    "licence_url": str,
    "attribution": str,
    "commercial_use": bool,
    "share_alike": bool,
    "use": str,
    "policy": str,
    "reason": str,
    "checked": datetime.date,
    "confidence": str,
}
POLICIES = {"bundle", "bundle-aggregates", "fetch", "exclude"}
DISTRIBUTIONS = {"closed", "open"}
# primary: feeds the pack; complement: fills gaps; fallback: used where no open source is
# adequate; validation: scoring only; rejected: audited and not used.
USES = {"primary", "complement", "fallback", "validation", "rejected"}
CONFIDENCE = {"high", "medium", "low"}
SHIPPED = {"bundle", "bundle-aggregates"}


def problems(source: dict, ids: set[str]) -> list[str]:
    sid = source.get("id", "?")
    found = [
        f"{sid}: field '{field}' missing or not {kind.__name__}"
        for field, kind in REQUIRED.items()
        if not isinstance(source.get(field), kind)
    ]
    if found:
        return found
    if source["policy"] not in POLICIES:
        found.append(f"{sid}: unknown policy '{source['policy']}'")
    if source["use"] not in USES:
        found.append(f"{sid}: unknown use '{source['use']}'")
    if source["use"] == "rejected" and source["policy"] != "exclude":
        found.append(f"{sid}: rejected sources must have policy exclude")
    if source["confidence"] not in CONFIDENCE:
        found.append(f"{sid}: unknown confidence '{source['confidence']}'")
    if not source["licence_url"].startswith("https://"):
        found.append(f"{sid}: licence_url must be https")
    if source["policy"] in SHIPPED:
        # Anything we redistribute must allow the commercial use our MIT/Apache licence permits.
        if not source["commercial_use"]:
            found.append(f"{sid}: shipped but commercial use not allowed")
        if source["confidence"] == "low":
            found.append(f"{sid}: shipped on a low-confidence licence reading")
        if source["share_alike"] and source.get("shipped_under") != source["licence"]:
            found.append(f"{sid}: share-alike data must set shipped_under = licence")
    if source.get("closed_source_only") and source["policy"] in SHIPPED:
        found.append(f"{sid}: closed-source-only data cannot be shipped")
    for link in ("replaces", "replaced_by"):
        if link in source and source[link] not in ids:
            found.append(f"{sid}: {link} names unknown source '{source[link]}'")
    return found


def main() -> int:
    manifest = tomllib.loads(MANIFEST.read_text())
    distribution = manifest.get("project", {}).get("distribution")
    sources = manifest.get("source", [])
    ids = [s.get("id") for s in sources]
    errors = [f"duplicate id '{i}'" for i in sorted({i for i in ids if ids.count(i) > 1})]
    if distribution not in DISTRIBUTIONS:
        errors.append(f"project.distribution must be one of {sorted(DISTRIBUTIONS)}")
    for source in sources:
        errors += problems(source, set(ids))
        # Opening the project requires every closed-source fallback to be resolved (ADR 0005).
        if distribution == "open" and source.get("closed_source_only"):
            errors.append(f"{source.get('id')}: closed-source-only data in an open distribution")
    for e in errors:
        print(e, file=sys.stderr)
    shipped = sum(s.get("policy") in SHIPPED for s in sources)
    closed = sum(bool(s.get("closed_source_only")) for s in sources)
    print(f"{distribution}: {len(sources)} sources, {shipped} shippable, {closed} closed-source-only, {len(errors)} problems")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
