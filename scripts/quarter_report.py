#!/usr/bin/env python3
"""App report renderer: editable text slides, converted locally by LibreOffice."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from xml.sax.saxutils import escape


def render(source):
    payload = json.loads(source.read_text())
    state = payload["company"]
    slides = [(state["name"], [payload["quarter"], f"Snapshot: {payload['generated_at']}",
        "One day represents one reporting year, with four six-hour quarters.",
        "Cash service budget: $0. Electricity and hardware excluded."])]
    if not state["jobs"]:
        slides.append(("Work queue", ["No company jobs have entered intake.", "No progress percentage or delivery date has been inferred."]))
    for job in state["jobs"]:
        intake = job["intake"]
        slides.append(("Current scope", [f"Run {job['run_id']}", f"State: {job['state']}", intake["idea"], f"Timeline: {intake['timeline']}"]))
        slides.append(("Acceptance milestones", intake["acceptance"].splitlines()))
        slides.append(("Task owners", ["Law and CTO review intake and staffing.",
            "PM owns milestones. Lead Architect owns the technical plan.",
            "Engineer produces the deliverable. QA reviews the criteria.",
            "Configured assignments are not evidence of completed work."]))
    slides.append(("Context and review", [f"{t['tier']}: {t['records']} context records" for t in state["tiers"]] +
        ["Tools remain quarantined until reviewed.", "Production requires executed checks and human authorization."]))
    for job in state.get("training",[])[:4]:
        metrics=job.get("progress",{}).get("metrics",{})
        lines=[f"Role: {job['manifest']['role']}",f"State: {job['state']}",f"Base: {job['manifest']['base_model']}"]
        if "baseline_test_loss" in metrics:
            lines.append(f"Held-out loss: {metrics['baseline_test_loss']:.4f} to {metrics['adapter_test_loss']:.4f}. Task improvement still requires independent evaluation.")
        slides.append(("Training Room",lines))
    for milestone in state.get("milestones",[])[:8]:
        slides.append(("Milestone evidence",[milestone["title"],f"Owner: {milestone['agent']}. State: {milestone['state']}",
            f"Deadline epoch milliseconds: {milestone.get('due_ms') or 'Unknown'}",milestone.get("evidence") or "Execution evidence pending."]))
    if state.get("meetings"):
        meeting=state["meetings"][0]
        slides.append(("CEO all-hands",[f"Meeting: {meeting['id']}",f"Run: {meeting['run_id']}",meeting["brief"][:600],"The original Scope of Work and acceptance gates remain in force."]))
    pages = []
    for title, paragraphs in slides:
        chunks = [str(p)[i:i+240] for p in paragraphs for i in range(0,len(str(p)),240)]
        for offset in range(0,max(1,len(chunks)),4):
            body = "".join(f'<text:p text:style-name="Body">{escape(text)}</text:p>' for text in chunks[offset:offset+4])
            pages.append(f'<draw:page draw:name="Slide {len(pages)+1}" draw:style-name="PageStyle" draw:master-page-name="Default">'
                f'<draw:frame draw:style-name="TextFrame" svg:x="1.8cm" svg:y="1.3cm" svg:width="30cm" svg:height="2.5cm"><draw:text-box>'
                f'<text:p text:style-name="Title">{escape(title)}</text:p></draw:text-box></draw:frame>'
                f'<draw:frame draw:style-name="TextFrame" svg:x="1.8cm" svg:y="4.3cm" svg:width="30cm" svg:height="13cm"><draw:text-box>{body}</draw:text-box></draw:frame></draw:page>')
    namespaces = {"office":"office", "draw":"drawing", "text":"text", "style":"style", "fo":"xsl-fo-compatible", "svg":"svg-compatible"}
    xmlns = " ".join(f'xmlns:{k}="urn:oasis:names:tc:opendocument:xmlns:{v}:1.0"' for k,v in namespaces.items())
    document = f'''<?xml version="1.0" encoding="utf-8"?>
<office:document {xmlns} office:version="1.3" office:mimetype="application/vnd.oasis.opendocument.presentation">
<office:automatic-styles>
<style:style style:name="TextFrame" style:family="graphic"><style:graphic-properties draw:fill="none" draw:stroke="none" draw:auto-grow-width="false" draw:auto-grow-height="false" fo:padding="0cm"/></style:style>
<style:style style:name="PageStyle" style:family="drawing-page"><style:drawing-page-properties draw:fill="solid" draw:fill-color="#f7f8fb"/></style:style>
<style:page-layout style:name="Layout"><style:page-layout-properties fo:page-width="33.867cm" fo:page-height="19.05cm" style:print-orientation="landscape"/></style:page-layout>
<style:style style:name="Title" style:family="paragraph"><style:text-properties fo:font-family="Arial" fo:font-size="32pt" fo:color="#19354b"/></style:style>
<style:style style:name="Body" style:family="paragraph"><style:paragraph-properties fo:margin-bottom="0.7cm"/><style:text-properties fo:font-family="Arial" fo:font-size="20pt" fo:color="#263746"/></style:style>
</office:automatic-styles>
<office:master-styles><style:master-page style:name="Default" style:page-layout-name="Layout"/></office:master-styles>
<office:body><office:presentation>{''.join(pages)}</office:presentation></office:body></office:document>'''
    executable = shutil.which("soffice")
    if not executable:
        raise RuntimeError("PPTX export requires local LibreOffice: brew install --cask libreoffice")
    with tempfile.TemporaryDirectory(prefix="gents-slides-") as temporary:
        directory = Path(temporary)
        draft = directory/(source.stem+".fodp")
        draft.write_text(document)
        subprocess.run([executable, "-env:UserInstallation="+(directory/"profile").as_uri(), "--headless",
            "--convert-to", "pptx", "--outdir", str(directory), str(draft)], check=True,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=90)
        target = source.with_suffix(".pptx")
        if target.is_symlink():
            raise ValueError("report target symlink rejected")
        shutil.copyfile(directory/(source.stem+".pptx"), target)
    print(target)


if __name__ == "__main__":
    render(Path(sys.argv[1]).resolve())
