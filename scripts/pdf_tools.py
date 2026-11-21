#!/usr/bin/env python3
"""Bounded PDF evidence extraction/rendering; never authors task content."""

import hashlib
import json
import subprocess
import sys
from pathlib import Path


class InspectionError(ValueError):
    def __init__(self, code, message):
        super().__init__(message)
        self.code = code


def open_pdf(path, reader_factory=None):
    if reader_factory is None:
        from pypdf import PdfReader

        reader_factory = PdfReader
    reader = reader_factory(path)
    encrypted = reader.is_encrypted
    # Normal password-free viewing is different from requiring an opening
    # password. Never guess a nonempty password or write a decrypted copy.
    if encrypted and not reader.decrypt(""):
        raise InspectionError(
            "password_required",
            "The PDF requires an authorized opening password; this tool does not guess passwords",
        )
    if encrypted and reader.are_permissions_valid is False:
        raise InspectionError(
            "permissions_integrity_failed", "The PDF permission integrity check failed"
        )
    permissions = reader.user_access_permissions
    text_allowed = not encrypted or bool(
        permissions and permissions & type(permissions).EXTRACT
    )
    return reader, {
        "encrypted": encrypted,
        "opened_without_supplied_password": encrypted,
        "text_extraction_allowed": text_allowed,
    }


def inspect(path, destination, start=1, count=4):
    path, destination = Path(path), Path(destination)
    if path.is_symlink() or not path.is_file() or path.suffix.lower() != ".pdf":
        raise InspectionError(
            "invalid_path", "Inspect an existing PDF without symlinks"
        )
    if not 1 <= count <= 8 or start < 1:
        raise InspectionError(
            "invalid_page_range", "page_start >= 1 and page_count must be 1..8"
        )
    reader, access = open_pdf(path)
    if start > len(reader.pages):
        raise InspectionError("invalid_page_range", "page_start exceeds PDF page count")
    with path.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    destination.mkdir(parents=True, exist_ok=True)
    end = min(len(reader.pages), start + count - 1)
    prefix = destination / f"page-{start}-{end}"
    subprocess.run(
        [
            "pdftoppm",
            "-f",
            str(start),
            "-l",
            str(end),
            "-scale-to",
            "1200",
            "-png",
            str(path),
            str(prefix),
        ],
        check=True,
        capture_output=True,
        timeout=120,
    )
    pages = []
    for number in range(start, end + 1):
        text = (
            (reader.pages[number - 1].extract_text() or "")
            if access["text_extraction_allowed"]
            else ""
        )
        pages.append(
            {
                "page": number,
                "text": text[:6000],
                "text_truncated": len(text) > 6000,
                "text_access": "permitted"
                if access["text_extraction_allowed"]
                else "restricted; use rendered pages for visual review",
            }
        )
    renders = sorted(
        destination.glob(f"page-{start}-{end}-*.png"),
        key=lambda image: int(image.stem.rsplit("-", 1)[1]),
    )
    if len(renders) != len(pages) or any(
        image.stat().st_size == 0 for image in renders
    ):
        raise InspectionError("render_incomplete", "PDF page rendering was incomplete")
    for page, image in zip(pages, renders):
        page["render"] = str(image)
    result = {
        "sha256": digest,
        "page_count": len(reader.pages),
        "inspected_pages": pages,
        "rendered": True,
        "layout_verified": False,
        "source_access": access,
        "note": "Text/render evidence only. Followability, real-part validity and stability still require independent task-specific review.",
    }
    (destination / f"inspection-{start}.json").write_text(json.dumps(result, indent=2))
    return result


if __name__ == "__main__":
    try:
        request = json.load(sys.stdin)
        print(
            json.dumps(
                inspect(
                    request["path"],
                    request["destination"],
                    request.get("page_start", 1),
                    request.get("page_count", 4),
                )
            )
        )
    except InspectionError as error:
        print(json.dumps({"error": {"code": error.code, "message": str(error)}}))
        sys.exit(1)
    except subprocess.CalledProcessError as error:
        print(
            json.dumps(
                {
                    "error": {
                        "code": "render_failed",
                        "message": f"pdftoppm failed with exit code {error.returncode}",
                    }
                }
            )
        )
        sys.exit(1)
    except Exception as error:
        print(
            json.dumps(
                {
                    "error": {
                        "code": "pdf_inspection_failed",
                        "message": "PDF parsing or a local dependency failed",
                        "exception_type": type(error).__name__,
                    }
                }
            )
        )
        sys.exit(1)
