#!/usr/bin/env python3
"""Render agent-authored JSON pages. Contains no task content or model calls."""
import hashlib
import json
import math
from pathlib import Path
import re
import sys
import tempfile
import textwrap


def number(value, lower=0, upper=516):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or not lower <= value <= upper:
        raise ValueError(f"drawing number must be finite and within {lower}..{upper}")
    return value


def text(value, limit):
    if not isinstance(value, str) or len(value) > limit:
        raise ValueError(f"text must be a string up to {limit} characters")
    # The built-in PDF font uses WinAnsi; fail rather than silently lose glyphs.
    value.encode("cp1252")
    return value


def color(value):
    if value is not None and (not isinstance(value, str) or not re.fullmatch(r"#[0-9a-fA-F]{6}", value)):
        raise ValueError("colors must be #RRGGBB or null")
    return value


def validate(spec):
    if not isinstance(spec, dict) or set(spec) - {"title", "pages"}:
        raise ValueError("document requires title and pages, with no extra fields")
    text(spec.get("title"), 160)
    pages = spec.get("pages")
    if not isinstance(pages, list) or not 1 <= len(pages) <= 100:
        raise ValueError("pages must contain 1..100 authored pages")
    for page in pages:
        if not isinstance(page, dict) or set(page) - {"title", "text", "drawing"}:
            raise ValueError("page fields: title, text (paragraph strings), drawing (shape objects)")
        text(page.get("title"), 100)
        paragraphs = page.get("text", [])
        if not isinstance(paragraphs, list) or len(paragraphs) > 20:
            raise ValueError("text must be an array of at most 20 paragraph strings")
        lines = sum(max(1, len(textwrap.wrap(text(p, 2000), 88))) + 1 for p in paragraphs)
        shapes = page.get("drawing", [])
        if not isinstance(shapes, list) or len(shapes) > 2000:
            raise ValueError("drawing must be an array of at most 2000 shapes")
        if lines > (19 if shapes else 43):
            raise ValueError("page text would overflow; split it into another page")
        for shape in shapes:
            validate_shape(shape)
    return spec


def validate_shape(shape):
    fields = {
        "rect": {"x", "y", "width", "height"},
        "ellipse": {"x", "y", "width", "height"},
        "line": {"x", "y", "x2", "y2"},
        "polygon": {"points"},
        "label": {"x", "y", "text", "size"},
    }
    if not isinstance(shape, dict) or shape.get("kind") not in fields:
        raise ValueError("shape kind must be rect, ellipse, line, polygon, or label")
    kind = shape["kind"]
    if set(shape) - (fields[kind] | {"kind", "fill", "stroke", "line_width"}):
        raise ValueError("unknown drawing field")
    color(shape.get("fill"))
    color(shape.get("stroke", "#263238"))
    number(shape.get("line_width", 1), 0, 10)
    if kind == "polygon":
        points = shape.get("points")
        if not isinstance(points, list) or not 3 <= len(points) <= 80:
            raise ValueError("polygon requires 3..80 [x,y] points")
        for point in points:
            if not isinstance(point, list) or len(point) != 2:
                raise ValueError("polygon point requires [x,y]")
            number(point[0]); number(point[1], 0, 360)
        return
    x, y = number(shape.get("x")), number(shape.get("y"), 0, 360)
    if kind in {"rect", "ellipse"}:
        width, height = number(shape.get("width")), number(shape.get("height"), 0, 360)
        if x + width > 516 or y + height > 360:
            raise ValueError("shape extends beyond the 516 x 360 drawing area")
    elif kind == "line":
        number(shape.get("x2")); number(shape.get("y2"), 0, 360)
    else:
        text(shape.get("text"), 200)
        number(shape.get("size", 10), 6, 24)


def render(source, output):
    from reportlab.lib.colors import HexColor
    from reportlab.pdfbase.pdfmetrics import stringWidth
    from reportlab.pdfgen.canvas import Canvas
    source, output = Path(source), Path(output)
    if source.is_symlink() or output.is_symlink() or source.suffix != ".json" or output.suffix != ".pdf":
        raise ValueError("source must be a JSON file and output a PDF, without symlinks")
    raw = source.read_bytes()
    if len(raw) > 2_000_000:
        raise ValueError("page specification exceeds 2 MB")
    spec = validate(json.loads(raw))
    output.parent.mkdir(parents=True, exist_ok=True)
    staged = None
    try:
        with tempfile.NamedTemporaryFile(dir=output.parent, suffix=".pdf", delete=False) as handle:
            staged = Path(handle.name)
        canvas = Canvas(str(staged), pagesize=(612, 792), pageCompression=1)
        canvas.setTitle(spec["title"])
        for index, page in enumerate(spec["pages"], 1):
            canvas.setFont("Helvetica-Bold", 18)
            if stringWidth(page["title"], "Helvetica-Bold", 18) > 516:
                raise ValueError("page title exceeds the available width; shorten it")
            canvas.drawString(48, 738, page["title"])
            canvas.setFont("Helvetica", 11)
            y = 706
            for paragraph in page.get("text", []):
                for line in textwrap.wrap(paragraph, 88) or [""]:
                    if stringWidth(line, "Helvetica", 11) > 516:
                        raise ValueError("text line exceeds the page width; shorten it")
                    canvas.drawString(48, y, line)
                    y -= 14
                y -= 14
            canvas.saveState()
            canvas.translate(48, 70)
            for shape in page.get("drawing", []):
                fill, stroke = shape.get("fill"), shape.get("stroke", "#263238")
                if fill: canvas.setFillColor(HexColor(fill))
                if stroke: canvas.setStrokeColor(HexColor(stroke))
                canvas.setLineWidth(shape.get("line_width", 1))
                kind = shape["kind"]
                if kind == "rect":
                    canvas.rect(shape["x"], shape["y"], shape["width"], shape["height"], stroke=bool(stroke), fill=bool(fill))
                elif kind == "ellipse":
                    canvas.ellipse(shape["x"], shape["y"], shape["x"]+shape["width"], shape["y"]+shape["height"], stroke=bool(stroke), fill=bool(fill))
                elif kind == "line":
                    canvas.line(shape["x"], shape["y"], shape["x2"], shape["y2"])
                elif kind == "polygon":
                    path = canvas.beginPath()
                    path.moveTo(*shape["points"][0])
                    for point in shape["points"][1:]: path.lineTo(*point)
                    path.close()
                    canvas.drawPath(path, stroke=bool(stroke), fill=bool(fill))
                else:
                    size = shape.get("size", 10)
                    if shape["x"] + stringWidth(shape["text"], "Helvetica", size) > 516 or shape["y"] + size > 360:
                        raise ValueError("drawing label extends beyond the drawing area")
                    canvas.setFillColor(HexColor(fill or stroke or "#263238"))
                    canvas.setFont("Helvetica", size)
                    canvas.drawString(shape["x"], shape["y"], shape["text"])
            canvas.restoreState()
            canvas.setFont("Helvetica", 9)
            canvas.setFillColor(HexColor("#546e7a"))
            canvas.drawRightString(564, 36, f"{index} / {len(spec['pages'])}")
            canvas.showPage()
        canvas.save()
        staged.replace(output)
    finally:
        if staged is not None and staged.exists(): staged.unlink()
    return {"sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
            "source_sha256": hashlib.sha256(raw).hexdigest(), "page_count": len(spec["pages"]),
            "bytes": output.stat().st_size, "quality_verified": False,
            "next": "pdf_inspect, then image_review; authoring is not independent QA"}


if __name__ == "__main__":
    try:
        request = json.load(sys.stdin)
        print(json.dumps(render(request["source"], request["output"])))
    except Exception as error:
        print(json.dumps({"error": str(error)}))
        sys.exit(1)
