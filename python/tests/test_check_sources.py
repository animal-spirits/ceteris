import datetime

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
