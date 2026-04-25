#!/usr/bin/env python3
"""Render a Markdown study document into a readable PDF without external tools.

This renderer intentionally supports only the subset used by the project study docs:

- headings (#, ##, ###)
- paragraphs
- flat bullet and numbered lists
- fenced code blocks
- horizontal rules

It uses Pillow to draw text onto A4 pages and saves a multi-page PDF.
"""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


PAGE_WIDTH = 1240
PAGE_HEIGHT = 1754
MARGIN_X = 110
MARGIN_TOP = 110
MARGIN_BOTTOM = 110
CONTENT_WIDTH = PAGE_WIDTH - (2 * MARGIN_X)


@dataclass
class Block:
    kind: str
    text: str = ""
    level: int = 0
    lines: list[str] | None = None
    marker: str = ""


def font_candidates(name: str) -> list[str]:
    return [
        f"/usr/share/fonts/truetype/dejavu/{name}",
        f"/usr/local/share/fonts/{name}",
    ]


def load_font(name: str, size: int) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    for candidate in font_candidates(name):
        if Path(candidate).exists():
            return ImageFont.truetype(candidate, size=size)
    return ImageFont.load_default()


FONTS = {
    "title": load_font("DejaVuSans-Bold.ttf", 44),
    "h1": load_font("DejaVuSans-Bold.ttf", 36),
    "h2": load_font("DejaVuSans-Bold.ttf", 31),
    "h3": load_font("DejaVuSans-Bold.ttf", 27),
    "body": load_font("DejaVuSans.ttf", 25),
    "code": load_font("DejaVuSansMono.ttf", 22),
    "small": load_font("DejaVuSans.ttf", 20),
    "page": load_font("DejaVuSans.ttf", 18),
}

COLORS = {
    "ink": (25, 30, 36),
    "muted": (94, 104, 118),
    "accent": (30, 74, 124),
    "rule": (180, 188, 198),
    "code_bg": (245, 247, 250),
}


def simplify_inline_markdown(text: str) -> str:
    text = re.sub(r"`([^`]+)`", r"\1", text)
    text = re.sub(r"\*\*([^*]+)\*\*", r"\1", text)
    text = re.sub(r"\*([^*]+)\*", r"\1", text)
    text = re.sub(r"\[([^\]]+)\]\([^)]+\)", r"\1", text)
    return text.strip()


def parse_markdown(text: str) -> list[Block]:
    blocks: list[Block] = []
    paragraph: list[str] = []
    code_lines: list[str] = []
    in_code = False

    def flush_paragraph() -> None:
        nonlocal paragraph
        if paragraph:
            blocks.append(Block(kind="paragraph", text=simplify_inline_markdown(" ".join(paragraph))))
            paragraph = []

    for raw_line in text.splitlines():
        line = raw_line.rstrip()
        stripped = line.strip()

        if stripped.startswith("```"):
            flush_paragraph()
            if in_code:
                blocks.append(Block(kind="code", lines=code_lines.copy()))
                code_lines.clear()
                in_code = False
            else:
                in_code = True
            continue

        if in_code:
            code_lines.append(line.rstrip("\n"))
            continue

        if not stripped:
            flush_paragraph()
            continue

        if re.fullmatch(r"-{3,}", stripped):
            flush_paragraph()
            blocks.append(Block(kind="rule"))
            continue

        heading_match = re.match(r"^(#{1,3})\s+(.*)$", stripped)
        if heading_match:
            flush_paragraph()
            hashes, content = heading_match.groups()
            blocks.append(
                Block(kind="heading", level=len(hashes), text=simplify_inline_markdown(content))
            )
            continue

        bullet_match = re.match(r"^[-*]\s+(.*)$", stripped)
        if bullet_match:
            flush_paragraph()
            blocks.append(Block(kind="bullet", text=simplify_inline_markdown(bullet_match.group(1)), marker="•"))
            continue

        number_match = re.match(r"^(\d+)\.\s+(.*)$", stripped)
        if number_match:
            flush_paragraph()
            blocks.append(
                Block(
                    kind="number",
                    text=simplify_inline_markdown(number_match.group(2)),
                    marker=f"{number_match.group(1)}.",
                )
            )
            continue

        paragraph.append(stripped)

    flush_paragraph()
    if code_lines:
        blocks.append(Block(kind="code", lines=code_lines.copy()))
    return blocks


def text_width(draw: ImageDraw.ImageDraw, text: str, font: ImageFont.ImageFont) -> float:
    return draw.textlength(text, font=font)


def wrap_words(
    draw: ImageDraw.ImageDraw,
    text: str,
    font: ImageFont.ImageFont,
    max_width: int,
) -> list[str]:
    words = text.split()
    if not words:
        return [""]

    lines: list[str] = []
    current = words[0]
    for word in words[1:]:
        candidate = f"{current} {word}"
        if text_width(draw, candidate, font) <= max_width:
            current = candidate
        else:
            lines.append(current)
            current = word
    lines.append(current)
    return lines


def wrap_code_line(
    draw: ImageDraw.ImageDraw,
    text: str,
    font: ImageFont.ImageFont,
    max_width: int,
) -> list[str]:
    if text_width(draw, text, font) <= max_width:
        return [text]

    pieces: list[str] = []
    current = ""
    for char in text:
        candidate = current + char
        if current and text_width(draw, candidate, font) > max_width:
            pieces.append(current)
            current = char
        else:
            current = candidate
    if current:
        pieces.append(current)
    return pieces


class Renderer:
    def __init__(self) -> None:
        self.pages: list[Image.Image] = []
        self.page_index = 0
        self.page = None
        self.draw = None
        self.y = MARGIN_TOP
        self.new_page()

    def new_page(self) -> None:
        self.page = Image.new("RGB", (PAGE_WIDTH, PAGE_HEIGHT), "white")
        self.draw = ImageDraw.Draw(self.page)
        self.pages.append(self.page)
        self.page_index += 1
        self.y = MARGIN_TOP

    def ensure_space(self, needed: int) -> None:
        if self.y + needed > PAGE_HEIGHT - MARGIN_BOTTOM:
            self.new_page()

    def line_height(self, font: ImageFont.ImageFont, extra: int = 0) -> int:
        bbox = font.getbbox("Hg")
        return (bbox[3] - bbox[1]) + extra

    def add_heading(self, text: str, level: int) -> None:
        font = {1: FONTS["title"], 2: FONTS["h1"], 3: FONTS["h2"]}.get(level, FONTS["h3"])
        fill = COLORS["accent"] if level <= 2 else COLORS["ink"]
        line_gap = {1: 14, 2: 10, 3: 8}.get(level, 6)
        lines = wrap_words(self.draw, text, font, CONTENT_WIDTH)
        needed = (len(lines) * self.line_height(font, line_gap)) + 18
        self.ensure_space(needed)
        for line in lines:
            self.draw.text((MARGIN_X, self.y), line, font=font, fill=fill)
            self.y += self.line_height(font, line_gap)
        self.y += 8

    def add_paragraph(self, text: str, indent: int = 0) -> None:
        font = FONTS["body"]
        lines = wrap_words(self.draw, text, font, CONTENT_WIDTH - indent)
        needed = (len(lines) * self.line_height(font, 8)) + 10
        self.ensure_space(needed)
        for line in lines:
            self.draw.text((MARGIN_X + indent, self.y), line, font=font, fill=COLORS["ink"])
            self.y += self.line_height(font, 8)
        self.y += 4

    def add_list_item(self, marker: str, text: str) -> None:
        font = FONTS["body"]
        marker_width = int(text_width(self.draw, f"{marker} ", font)) + 10
        lines = wrap_words(self.draw, text, font, CONTENT_WIDTH - marker_width)
        needed = (len(lines) * self.line_height(font, 7)) + 8
        self.ensure_space(needed)
        self.draw.text((MARGIN_X, self.y), marker, font=font, fill=COLORS["ink"])
        for index, line in enumerate(lines):
            x = MARGIN_X + marker_width
            self.draw.text((x, self.y), line, font=font, fill=COLORS["ink"])
            self.y += self.line_height(font, 7)
        self.y += 2

    def add_rule(self) -> None:
        self.ensure_space(28)
        y = self.y + 10
        self.draw.line((MARGIN_X, y, PAGE_WIDTH - MARGIN_X, y), fill=COLORS["rule"], width=2)
        self.y += 28

    def add_code(self, lines: list[str]) -> None:
        font = FONTS["code"]
        wrapped: list[str] = []
        for line in lines:
            wrapped.extend(wrap_code_line(self.draw, line, font, CONTENT_WIDTH - 30))
        block_height = (len(wrapped) * self.line_height(font, 6)) + 24
        self.ensure_space(block_height)
        self.draw.rounded_rectangle(
            (MARGIN_X, self.y, PAGE_WIDTH - MARGIN_X, self.y + block_height - 10),
            radius=12,
            fill=COLORS["code_bg"],
            outline=COLORS["rule"],
            width=1,
        )
        self.y += 14
        for line in wrapped:
            self.draw.text((MARGIN_X + 15, self.y), line or " ", font=font, fill=COLORS["ink"])
            self.y += self.line_height(font, 6)
        self.y += 10

    def add_page_numbers(self) -> None:
        total = len(self.pages)
        for index, page in enumerate(self.pages, start=1):
            draw = ImageDraw.Draw(page)
            label = f"Página {index} / {total}"
            width = text_width(draw, label, FONTS["page"])
            draw.text(
                ((PAGE_WIDTH - width) / 2, PAGE_HEIGHT - 60),
                label,
                font=FONTS["page"],
                fill=COLORS["muted"],
            )


def render_markdown_to_pdf(source_path: Path, pdf_path: Path) -> None:
    text = source_path.read_text(encoding="utf-8")
    blocks = parse_markdown(text)
    renderer = Renderer()

    for block in blocks:
        if block.kind == "heading":
            renderer.add_heading(block.text, block.level)
        elif block.kind == "paragraph":
            renderer.add_paragraph(block.text)
        elif block.kind == "bullet":
            renderer.add_list_item(block.marker, block.text)
        elif block.kind == "number":
            renderer.add_list_item(block.marker, block.text)
        elif block.kind == "rule":
            renderer.add_rule()
        elif block.kind == "code":
            renderer.add_code(block.lines or [])

    renderer.add_page_numbers()
    images = [page.convert("RGB") for page in renderer.pages]
    pdf_path.parent.mkdir(parents=True, exist_ok=True)
    images[0].save(pdf_path, save_all=True, append_images=images[1:], resolution=150.0)


def main(argv: list[str]) -> int:
    if len(argv) not in {2, 3}:
        print(
            "usage: export_study_pdf.py <input.md> [output.pdf]",
            file=sys.stderr,
        )
        return 2

    source = Path(argv[1]).resolve()
    if len(argv) == 3:
        output = Path(argv[2]).resolve()
    else:
        output = source.with_suffix(".pdf")

    render_markdown_to_pdf(source, output)
    print(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
