"""Protocol and security unit tests (no network)."""

from __future__ import annotations

import pytest

from coreside_crawler.errors import CrawlError
from coreside_crawler.protocol import parse_line, validate_request
from coreside_crawler.security import validate_public_http_url
from coreside_crawler.discovery.ranking import rank_candidates
from coreside_crawler.extraction import bound_text_for_model
from coreside_crawler.media import normalize_media


def test_parse_and_validate_ok():
    line = '{"protocolVersion":"1","requestId":"r1","type":"health","payload":{}}'
    data = parse_line(line)
    rid, typ, payload, err = validate_request(data)
    assert err is None
    assert rid == "r1"
    assert typ == "health"


def test_malformed_line():
    assert parse_line("not-json") is None


def test_bad_protocol_version():
    rid, typ, payload, err = validate_request(
        {"protocolVersion": "9", "requestId": "x", "type": "health", "payload": {}}
    )
    assert err and "protocolVersion" in err


def test_ssrf_blocks():
    with pytest.raises(CrawlError):
        validate_public_http_url("file:///etc/passwd")
    with pytest.raises(CrawlError):
        validate_public_http_url("http://127.0.0.1/")
    with pytest.raises(CrawlError):
        validate_public_http_url("http://192.168.1.1/")
    with pytest.raises(CrawlError):
        validate_public_http_url("http://169.254.169.254/")
    with pytest.raises(CrawlError):
        validate_public_http_url("http://100.64.0.1/")
    with pytest.raises(CrawlError):
        validate_public_http_url("javascript:alert(1)")
    assert validate_public_http_url("https://1.1.1.1/")


def test_loopback_requires_env_gate(monkeypatch):
    monkeypatch.delenv("CORESIDE_CRAWLER_ALLOW_LOOPBACK", raising=False)
    with pytest.raises(CrawlError):
        validate_public_http_url("http://127.0.0.1/", allow_loopback_for_tests=True)
    monkeypatch.setenv("CORESIDE_CRAWLER_ALLOW_LOOPBACK", "1")
    assert validate_public_http_url("http://127.0.0.1/", allow_loopback_for_tests=True)


def test_ranking_dedupes_and_limits():
    cands = [
        {"url": "https://a.example/docs", "title": "docs rust", "snippet": "rust guide", "score": 0.1},
        {"url": "https://a.example/docs", "title": "dup", "snippet": "", "score": 9},
        {"url": "https://b.example/other", "title": "unrelated", "snippet": "", "score": 5},
    ]
    ranked = rank_candidates(cands, query="rust docs", limit=5)
    assert len(ranked) == 2
    assert ranked[0]["url"].endswith("/docs")


def test_bound_text():
    big = "alpha " * 20000
    out = bound_text_for_model(big, query="alpha")
    assert out["truncated"] is True
    assert len(out["markdown"]) < len(big)


class _FakeResult:
    url = "https://example.com/page"
    media = {
        "images": [
            {"src": "/tiny.png", "width": 10, "height": 10},
            {"src": "https://cdn.example.com/photo.jpg", "width": 1200, "height": 800, "alt": "photo"},
            {"src": "data:image/png;base64,xxx"},
        ],
        "videos": [{"src": "https://cdn.example.com/clip.mp4"}],
    }


def test_media_filters_tiny_and_data():
    media = normalize_media(_FakeResult())
    assert len(media["images"]) == 1
    assert media["images"][0]["wallpaperCandidate"] is True
    assert media["videos"][0]["kind"] == "direct"
