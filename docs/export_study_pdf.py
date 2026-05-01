#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later

"""Render a Markdown study document into a readable PDF and optional EPUB.

This renderer intentionally supports only the subset used by the project study docs:

- headings (#, ##, ###)
- paragraphs
- flat bullet and numbered lists
- fenced code blocks
- horizontal rules

PDF output uses Pillow to draw text onto A4 pages and saves a multi-page PDF.
EPUB output packages the same parsed block model into one XHTML chapter plus a TOC.
"""

from __future__ import annotations

import re
import sys
import unicodedata
import zipfile
from dataclasses import dataclass
from html import escape
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


PAGE_WIDTH = 1240
PAGE_HEIGHT = 1754
MARGIN_X = 110
MARGIN_TOP = 110
MARGIN_BOTTOM = 110
CONTENT_WIDTH = PAGE_WIDTH - (2 * MARGIN_X)

EPUB_CONTAINER_XML = """<?xml version="1.0" encoding="utf-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"""

EPUB_CSS = """
body {
  font-family: serif;
  line-height: 1.55;
  margin: 0 auto;
  max-width: 44em;
  padding: 1.2em;
  color: #18202a;
}

h1, h2, h3 {
  color: #1f4a7c;
  line-height: 1.2;
  margin-top: 1.5em;
}

h1 {
  font-size: 1.85em;
}

h2 {
  font-size: 1.45em;
}

h3 {
  font-size: 1.18em;
  color: #18202a;
}

p, li {
  font-size: 1em;
}

pre {
  background: #f5f7fa;
  border: 1px solid #c5cdd8;
  border-radius: 0.5em;
  overflow-x: auto;
  padding: 0.9em;
  white-space: pre-wrap;
}

code {
  font-family: monospace;
}

hr {
  border: 0;
  border-top: 1px solid #c5cdd8;
  margin: 1.8em 0;
}

nav ol {
  list-style: none;
  padding-left: 0;
}

nav li {
  margin: 0.25em 0;
}

nav li.lvl-2 {
  padding-left: 1em;
}

nav li.lvl-3 {
  padding-left: 2em;
}
"""


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
        for line in lines:
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


def extract_title(blocks: list[Block], fallback: str) -> str:
    for block in blocks:
        if block.kind == "heading" and block.text:
            return block.text
    return fallback.replace("-", " ").strip() or "Study Document"


def slugify(text: str, seen: dict[str, int]) -> str:
    normalized = unicodedata.normalize("NFKD", text)
    ascii_text = normalized.encode("ascii", "ignore").decode("ascii").lower()
    slug = re.sub(r"[^a-z0-9]+", "-", ascii_text).strip("-") or "section"
    seen[slug] = seen.get(slug, 0) + 1
    if seen[slug] == 1:
        return slug
    return f"{slug}-{seen[slug]}"


def render_blocks_to_xhtml(blocks: list[Block], title: str) -> tuple[str, list[tuple[int, str, str]]]:
    heading_ids: dict[str, int] = {}
    toc: list[tuple[int, str, str]] = []
    parts = [
        '<?xml version="1.0" encoding="utf-8"?>',
        '<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="es" lang="es">',
        "<head>",
        '  <meta charset="utf-8" />',
        f"  <title>{escape(title)}</title>",
        '  <link rel="stylesheet" type="text/css" href="../styles/book.css" />',
        "</head>",
        "<body>",
    ]

    list_kind: str | None = None

    def close_list() -> None:
        nonlocal list_kind
        if list_kind == "ul":
            parts.append("</ul>")
        elif list_kind == "ol":
            parts.append("</ol>")
        list_kind = None

    for block in blocks:
        if block.kind in {"bullet", "number"}:
            wanted = "ul" if block.kind == "bullet" else "ol"
            if list_kind != wanted:
                close_list()
                parts.append(f"<{wanted}>")
                list_kind = wanted
            parts.append(f"<li>{escape(block.text)}</li>")
            continue

        close_list()

        if block.kind == "heading":
            level = block.level if 1 <= block.level <= 3 else 3
            section_id = slugify(block.text, heading_ids)
            toc.append((level, block.text, section_id))
            parts.append(f'<h{level} id="{section_id}">{escape(block.text)}</h{level}>')
        elif block.kind == "paragraph":
            parts.append(f"<p>{escape(block.text)}</p>")
        elif block.kind == "rule":
            parts.append("<hr />")
        elif block.kind == "code":
            code = "\n".join(block.lines or [])
            parts.append(f"<pre><code>{escape(code)}</code></pre>")

    close_list()
    parts.append("</body>")
    parts.append("</html>")
    return "\n".join(parts), toc


def render_nav_xhtml(title: str, toc: list[tuple[int, str, str]]) -> str:
    items = []
    for level, text, section_id in toc:
        items.append(
            f'    <li class="lvl-{level}"><a href="text/content.xhtml#{section_id}">{escape(text)}</a></li>'
        )

    return "\n".join(
        [
            '<?xml version="1.0" encoding="utf-8"?>',
            '<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="es" lang="es">',
            "<head>",
            '  <meta charset="utf-8" />',
            f"  <title>{escape(title)} - Índice</title>",
            '  <link rel="stylesheet" type="text/css" href="styles/book.css" />',
            "</head>",
            "<body>",
            f"  <h1>{escape(title)}</h1>",
            '  <nav epub:type="toc" id="toc">',
            "    <ol>",
            *items,
            "    </ol>",
            "  </nav>",
            "</body>",
            "</html>",
        ]
    )


def render_package_opf(title: str, identifier: str) -> str:
    return f"""<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="bookid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="bookid">{escape(identifier)}</dc:identifier>
    <dc:title>{escape(title)}</dc:title>
    <dc:language>es</dc:language>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="content" href="text/content.xhtml" media-type="application/xhtml+xml"/>
    <item id="css" href="styles/book.css" media-type="text/css"/>
  </manifest>
  <spine>
    <itemref idref="content"/>
  </spine>
</package>
"""


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


def render_markdown_to_epub(source_path: Path, epub_path: Path) -> None:
    text = source_path.read_text(encoding="utf-8")
    blocks = parse_markdown(text)
    title = extract_title(blocks, source_path.stem)
    content_xhtml, toc = render_blocks_to_xhtml(blocks, title)
    nav_xhtml = render_nav_xhtml(title, toc)
    identifier = slugify(title, {})
    package_opf = render_package_opf(title, identifier)

    epub_path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(epub_path, "w") as archive:
        mimetype = zipfile.ZipInfo("mimetype")
        mimetype.compress_type = zipfile.ZIP_STORED
        archive.writestr(mimetype, "application/epub+zip")
        archive.writestr("META-INF/container.xml", EPUB_CONTAINER_XML)
        archive.writestr("OEBPS/content.opf", package_opf)
        archive.writestr("OEBPS/nav.xhtml", nav_xhtml)
        archive.writestr("OEBPS/text/content.xhtml", content_xhtml)
        archive.writestr("OEBPS/styles/book.css", EPUB_CSS)


def usage() -> str:
    return "usage: export_study_pdf.py <input.md> [output.pdf] [--epub [output.epub]]"


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(usage(), file=sys.stderr)
        return 2

    positionals: list[str] = []
    wants_epub = False
    epub_arg: str | None = None

    index = 1
    while index < len(argv):
        argument = argv[index]
        if argument == "--epub":
            wants_epub = True
            if index + 1 < len(argv) and not argv[index + 1].startswith("--"):
                epub_arg = argv[index + 1]
                index += 1
        else:
            positionals.append(argument)
        index += 1

    if not positionals or len(positionals) > 2:
        print(usage(), file=sys.stderr)
        return 2

    source = Path(positionals[0]).resolve()
    if not source.exists():
        print(f"input markdown not found: {source}", file=sys.stderr)
        return 1

    pdf_path = Path(positionals[1]).resolve() if len(positionals) == 2 else source.with_suffix(".pdf")
    outputs = []

    render_markdown_to_pdf(source, pdf_path)
    outputs.append(str(pdf_path))

    if wants_epub:
        epub_path = Path(epub_arg).resolve() if epub_arg else source.with_suffix(".epub")
        render_markdown_to_epub(source, epub_path)
        outputs.append(str(epub_path))

    print("\n".join(outputs))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
