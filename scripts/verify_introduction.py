"""Verify packaged local pages, links, metadata, structured content and CSP hashes."""

import base64
import hashlib
import json
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1] / "site/dist"


class Page(HTMLParser):
    def __init__(self, path):
        super().__init__()
        self.path, self.ids, self.links, self.meta = path, set(), [], {}
        self.h1 = 0
        self.structured = []
        self.in_json = False
        self.json_text = ""
        self.feed(path.read_text())

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if "id" in a:
            assert a["id"] not in self.ids, (self.path.name, "duplicate ID")
            self.ids.add(a["id"])
        if tag == "html":
            assert a.get("lang") == "en"
        if tag == "h1":
            self.h1 += 1
        if tag == "meta":
            self.meta[a.get("name", a.get("http-equiv", a.get("property")))] = a.get(
                "content"
            )
        if tag in ("a", "link"):
            self.links.append(a.get("href", ""))
        if tag in ("img", "script") and "src" in a:
            assert not urlsplit(a["src"]).scheme, "Unexpected remote asset"
            self.links.append(a["src"])
        if tag == "img":
            assert a.get("alt"), "Image needs an accessible description"
        if tag == "script" and a.get("type") == "application/ld+json":
            self.in_json = True
            self.json_text = ""
        if tag == "form":
            assert not a.get("action"), "Introduction cannot submit task data"

    def handle_data(self, data):
        if self.in_json:
            self.json_text += data

    def handle_endtag(self, tag):
        if tag == "script" and self.in_json:
            self.structured.append((self.json_text, json.loads(self.json_text)))
            self.in_json = False


pages = {p.name: Page(p) for p in ROOT.glob("*.html")}
info = json.loads((ROOT / "build-info.json").read_text())
assert set(pages) == set(info["pages"]) and len(pages) == 10
for page in pages.values():
    assert page.h1 == 1 and "main" in page.ids
    assert page.meta["robots"] == (
        "index,follow,max-image-preview:large"
        if info["indexable"]
        else "noindex,nofollow"
    )
    assert page.meta["description"] and page.meta["og:title"]
    policy = page.meta["Content-Security-Policy"]
    assert (
        "connect-src 'self'"
        if page.path.name == "support.html"
        else "connect-src 'none'"
    ) in policy and "form-action 'none'" in policy
    for raw, structured in page.structured:
        digest = base64.b64encode(hashlib.sha256(raw.encode()).digest()).decode()
        assert f"'sha256-{digest}'" in policy
        assert structured[0]["@type"] == (
            "TechArticle"
            if page.path.name == "cookbook.html"
            else "SoftwareApplication"
        )
    for link in page.links:
        parts = urlsplit(link)
        if parts.scheme:
            assert parts.scheme in ["https", "mailto"] or (
                parts.scheme == "http" and parts.netloc == "127.0.0.1:8788"
            ), "Only explicit loopback app navigation may use HTTP"
            continue
        target = ROOT / (parts.path or page.path.name)
        assert target.is_file(), (page.path.name, link)
        if parts.fragment:
            assert parts.fragment in pages[target.name].ids, (page.path.name, link)
assert (ROOT / "robots.txt").read_text() == (
    "User-agent: *\nAllow: /\nDisallow: /api/\nSitemap: https://metivolve.com/sitemap.xml\n"
    if info["indexable"]
    else "User-agent: *\nDisallow: /\n"
)
import xml.etree.ElementTree as ET

sitemap = ET.parse(ROOT / "sitemap.xml")
locations = [
    node.text
    for node in sitemap.findall(".//{http://www.sitemaps.org/schemas/sitemap/0.9}loc")
]
assert len(locations) == len(pages) and len(set(locations)) == len(pages)
assert all(
    url is not None and url.startswith("https://metivolve.com/") for url in locations
)
security = (ROOT / ".well-known/security.txt").read_text()
assert "Canonical: https://metivolve.com/.well-known/security.txt" in security
assert "Contact: mailto:robertjmonzingo@gmail.com" in security
from datetime import UTC, datetime

expiry = next(
    line.split(": ", 1)[1]
    for line in security.splitlines()
    if line.startswith("Expires: ")
)
assert datetime.fromisoformat(expiry) > datetime.now(UTC), (
    "Renew security.txt before it expires"
)
for name in ("index.html", "guide.html"):
    assert "https://github.com/Monzingo89/gents-rust-lab" in pages[name].links
assert not any(
    p.suffix in [".sqlite3", ".gguf", ".safetensors"] for p in ROOT.rglob("*")
)
print(
    "PASS: ten introduction pages; local links/anchors, accessible labels, metadata, structured content and CSP hashes; explicit preview/production indexing policy; no remote assets; only support allows same-origin API requests"
)
