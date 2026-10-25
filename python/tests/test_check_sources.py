import datetime

import pytest

from ceteris.packs import check_sources


def source(**overrides) -> dict:
    base = {
        "id": "s",
        "name": "S",
        "provider": "P",
        "version": "1",
        "role": "test",
        "licence": "CC-BY-4.0",
        "licence_url": "https://example.org/licence",
        "attribution": "P",
        "commercial_use": True,
        "share_alike": False,
        "use": "primary",
        "policy": "fetch",
        "reason": "test",
        "checked": datetime.date(2026, 10, 24),
        "confidence": "high",
    }
    return base | overrides


def test_research_profile_allows_non_commercial_and_closed_source_only():
    s = source(commercial_use=False, closed_source_only=True, use="validation")
    assert check_sources.profile_problems(s, "research") == []


def test_release_profile_rejects_non_commercial_closed_and_low_confidence():
    found = check_sources.profile_problems(source(commercial_use=False, closed_source_only=True, confidence="low"), "release")
    assert len(found) == 3


def test_release_profile_accepts_a_commercial_confident_source():
    assert check_sources.profile_problems(source(), "release") == []


def test_no_profile_reads_an_excluded_source():
    s = source(use="rejected", policy="exclude")
    for profile in check_sources.PROFILES:
        assert check_sources.profile_problems(s, profile)


def test_pack_sources_are_in_the_manifest_and_the_current_pack_passes_both_profiles():
    assert check_sources.pack_sources()
    for profile in sorted(check_sources.PROFILES):
        assert check_sources.main(["--profile", profile]) == 0


MANIFEST = """
[project]
distribution = "closed"

[[source]]
id = "pack-input"
name = "P"
provider = "P"
version = "1"
role = "test"
licence = "CC-BY-4.0"
licence_url = "https://example.org/licence"
attribution = "P"
commercial_use = true
share_alike = false
use = "primary"
policy = "fetch"
reason = "test"
checked = 2026-10-25
confidence = "high"

[[source]]
id = "sample"
name = "S"
provider = "S"
version = "1"
role = "test"
licence = "LicenseRef-all-rights-reserved"
licence_url = "https://example.org/terms"
attribution = "S"
commercial_use = false
share_alike = false
use = "{use}"
policy = "fetch"
reason = "test"
checked = 2026-10-25
confidence = "medium"
closed_source_only = true
"""


@pytest.fixture
def synthetic(tmp_path, monkeypatch):
    """A manifest with one commercial pack input and one non-commercial sample, and the file lists naming them."""

    def write(use: str, sample_in: str) -> None:
        (tmp_path / "sources.toml").write_text(MANIFEST.replace("{use}", use))
        files = {"fetch.toml": ["pack-input"], "validation.toml": []}
        files[sample_in].append("sample")
        for name, ids in files.items():
            (tmp_path / name).write_text("".join(f'[[file]]\nsource = "{i}"\nname = "{i}.csv"\nurl = "u"\n\n' for i in ids))

    monkeypatch.setattr(check_sources, "MANIFEST", tmp_path / "sources.toml")
    monkeypatch.setattr(check_sources, "FETCH", tmp_path / "fetch.toml")
    monkeypatch.setattr(check_sources, "VALIDATION", tmp_path / "validation.toml")
    monkeypatch.setattr(check_sources, "COUNTY_LOCK", tmp_path / "counties.lock.toml")
    return write


def test_a_validation_sample_outside_the_pack_passes_the_release_profile(synthetic):
    synthetic("validation", "validation.toml")
    assert check_sources.pack_sources() == {"pack-input"}
    for profile in sorted(check_sources.PROFILES):
        assert check_sources.main(["--profile", profile]) == 0


def test_the_same_sample_read_by_the_pack_fails_only_the_release_profile(synthetic):
    synthetic("validation", "fetch.toml")
    assert check_sources.main(["--profile", "research"]) == 0
    assert check_sources.main(["--profile", "release"]) == 1


def test_validation_files_must_belong_to_validation_sources(synthetic):
    synthetic("complement", "validation.toml")
    assert check_sources.main([]) == 1
