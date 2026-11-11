#!/usr/bin/env python3
"""Writes src/look/icons.rs from Electron's own drawings, so the Rust app draws the same glyphs and marks.

    python3 scripts/marks.py [--electron <checkout>]          rewrite src/look/icons.rs
    python3 scripts/marks.py --check [--electron <checkout>]  fail if it no longer matches Electron

Sources, all in Electron's renderer (apps/desktop/src/renderer/src):
- components/Icon.tsx: the shell's icon set, 24 box, 1.7 stroke, round caps and joins (Lucide's and its own, ISC).
- runtimeMarks.ts: each agent's mark, with its own box; Gemini's gradient takes its stops from tokens.css.
- assets/locust-mark.svg and locust-wordmark.svg: drawn in the text's colour, centred in a square box.
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ARGS = sys.argv[1:]
CHECK = "--check" in ARGS
ELECTRON = ARGS[ARGS.index("--electron") + 1] if "--electron" in ARGS else os.environ.get(
    "LOCUST_ELECTRON_CHECKOUT", os.path.join(HERE, "..", "..", ".."))
RENDERER = os.path.join(ELECTRON, "apps", "desktop", "src", "renderer", "src")
OUT = os.path.join(HERE, "..", "src", "look", "icons.rs")

STROKE = 'fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"'


def read(*parts):
    with open(os.path.join(RENDERER, *parts), encoding="utf-8") as f:
        return f.read()


def without_comments(src):
    src = re.sub(r"/\*.*?\*/", "", src, flags=re.S)
    return re.sub(r"^\s*//.*$", "", src, flags=re.M)


def const_name(name):
    return re.sub(r"[^A-Z0-9]", "_", name.upper())


def icons():
    src = without_comments(read("components", "Icon.tsx"))
    body = src[src.index("const paths: Record<IconName, ReactNode> = {") :]
    body = body[body.index("{") + 1 : body.index("\n  }\n")]
    found = {}
    for m in re.finditer(r"^\s*(?:'([a-z-]+)'|([a-z]+)):\s*(.*?)(?=^\s*(?:'[a-z-]+'|[a-z]+):\s|\Z)", body, flags=re.S | re.M):
        name = m.group(1) or m.group(2)
        jsx = m.group(3).strip().rstrip(",").strip()
        if jsx.startswith("("):
            jsx = jsx[1:].rstrip().rstrip(")").strip()
        jsx = jsx.replace("<>", "").replace("</>", "")
        jsx = re.sub(r"\s+", " ", jsx).strip()
        elements = re.findall(r"<[a-z]+\b[^>]*/>", jsx)
        assert elements, f"no elements in icon {name}"
        styled = []
        for el in elements:
            if 'fill="currentColor"' in el:
                styled.append(el)
            else:
                styled.append(el.replace("/>", f" {STROKE}/>").replace("  ", " "))
        found[name] = "".join(styled)
    names = re.findall(r"\|\s*'([a-z-]+)'", src[: src.index("export function Icon")])
    missing = sorted(set(names) - set(found))
    assert not missing, f"IconName lists icons with no drawing: {missing}"
    return dict(sorted(found.items()))


def tokens():
    css = without_comments(read("tokens.css"))
    root = css[css.index("\n:root {") :]
    root = root[: root.index("\n}")]
    return dict(re.findall(r"--(lc-[a-z0-9-]+):\s*([^;]+);", root))


def view_box(text, default="0 0 24 24"):
    x, y, w, h = (float(v) for v in (text or default).split())
    return x, y, w, h


def boxed(inner, x, y, w, h):
    """The drawing in a square box `max(w, h)` wide, centred, as rune::Icon takes a square viewBox from 0."""
    side = max(w, h)
    dx, dy = -x + (side - w) / 2, -y + (side - h) / 2
    return side, f'<g transform="translate({dx:g} {dy:g})">{inner}</g>'


def runtime_marks():
    src = without_comments(read("runtimeMarks.ts"))
    table = src[src.index("RUNTIME_MARKS") :]
    tok = tokens()
    found = {}
    for m in re.finditer(r"^\s{2}([a-z]+): \{(.*?)\n?\s*source:", table, flags=re.S | re.M):
        name, block = m.group(1), m.group(2)
        paths = re.findall(r'"(M[^"]+)"|"(m[^"]+)"', block)
        paths = [a or b for a, b in paths]
        vb = re.search(r"viewBox: '([^']+)'", block)
        x, y, w, h = view_box(vb.group(1) if vb else None)
        grad = re.search(r"gradient: \{\s*transform: '([^']+)',\s*stops: \[(.*?)\]", block, flags=re.S)
        if grad:
            stops = "".join(
                f'<stop offset="{off}" stop-color="{tok[t.lstrip("-")].strip()}"/>'
                for off, t in re.findall(r"offset: ([0-9.]+), token: '([^']+)'", grad.group(2))
            )
            defs = f'<defs><radialGradient id="g" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="{grad.group(1)}">{stops}</radialGradient></defs>'
            inner = defs + "".join(f'<path d="{d}" fill="url(#g)"/>' for d in paths)
        else:
            inner = "".join(f'<path d="{d}" fill="currentColor"/>' for d in paths)
        found[name] = boxed(inner, x, y, w, h)
    assert len(found) >= 9, f"found only {sorted(found)}"
    return dict(sorted(found.items()))


def svg_file(name):
    svg = read("assets", name)
    vb = re.search(r'viewBox="([^"]+)"', svg).group(1)
    inner = svg[svg.index(">", svg.index("<svg")) + 1 : svg.rindex("</svg>")]
    inner = re.sub(r"<title>.*?</title>", "", inner, flags=re.S).strip()
    inner = inner.replace('fill="#FFFFFF"', 'fill="currentColor"').replace('fill="#ffffff"', 'fill="currentColor"')
    inner = re.sub(r"\s*\n\s*", "", inner)
    return boxed(inner, *view_box(vb))


def rust_str(s):
    hashes = "#"
    while f'"{hashes}' in s:
        hashes += "#"
    return f'r{hashes}"{s}"{hashes}'


def render():
    out = [
        "//! Generated by scripts/marks.py from Electron's own drawings. Do not edit: change Electron's, then run",
        "//! `python3 scripts/marks.py` (CI runs it with --check).",
        "#![allow(dead_code)]",
        "use rune::Icon;",
        "",
    ]
    icon_list = icons()
    for name, body in icon_list.items():
        out.append(f'pub static {const_name(name)}: Icon = Icon {{ name: "{name}", view: 24.0, body: {rust_str(body)} }};')
    out.append("")
    out.append("/// Every icon of `Icon.tsx`, by its name there.")
    out.append("pub static ALL: &[&Icon] = &[" + ", ".join(f"&{const_name(n)}" for n in icon_list) + "];")
    out.append("")
    marks = runtime_marks()
    for name, (side, body) in marks.items():
        out.append(f'pub static MARK_{const_name(name)}: Icon = Icon {{ name: "mark-{name}", view: {side:g}.0, body: {rust_str(body)} }};'.replace(f"{side:g}.0", f"{float(side)!r}"))
    out.append("")
    out.append("/// Each agent's mark, by its runtime id (`runtimeMarks.ts`).")
    out.append("pub fn mark(runtime: &str) -> Option<&'static Icon> {")
    out.append("    match runtime {")
    for name in marks:
        out.append(f'        "{name}" => Some(&MARK_{const_name(name)}),')
    out.append("        _ => None,")
    out.append("    }")
    out.append("}")
    out.append("")
    out.append("pub static RUNTIMES: &[&str] = &[" + ", ".join(f'"{n}"' for n in marks) + "];")
    out.append("")
    for const, file, what in [("LOCUST_MARK", "locust-mark.svg", "The Locust mark"), ("LOCUST_WORDMARK", "locust-wordmark.svg", "The Locust wordmark")]:
        side, body = svg_file(file)
        out.append(f"/// {what} (`assets/{file}`), centred in a square box, in the text's colour.")
        out.append(f'pub static {const}: Icon = Icon {{ name: "{file[:-4]}", view: {float(side)!r}, body: {rust_str(body)} }};')
    return "\n".join(out) + "\n"


def main():
    text = render()
    if CHECK:
        with open(OUT, encoding="utf-8") as f:
            have = f.read()
        if have != text:
            print("src/look/icons.rs no longer matches Electron's drawings: run python3 scripts/marks.py", file=sys.stderr)
            sys.exit(1)
        icon_count = text[text.index("pub static ALL"):].split("\n", 1)[0].split("= &[", 1)[1].count("&")
        mark_count = text.count('name: "mark-')
        print(f"src/look/icons.rs matches Electron's drawings ({icon_count} icons, {mark_count} agent marks, the Locust mark and wordmark)")
        return
    with open(OUT, "w", encoding="utf-8") as f:
        f.write(text)
    print(f"wrote {os.path.relpath(OUT)}")


main()
