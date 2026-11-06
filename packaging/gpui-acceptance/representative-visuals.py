#!/usr/bin/env python3
"""Build review inputs from verified candidate pixels; records no review verdict."""
import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True)
    parser.add_argument("--candidate-sha", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    root, output = Path(args.evidence).resolve(), Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    catalog = json.loads((REPO / "docs/gpui-migration/inventory.json").read_text())
    styles = {s["id"]: s["renderer"] for s in catalog["styles"]}
    options, pieces = {}, set()
    for file in sorted(root.rglob("manifest.json")):
        data = json.loads(file.read_text())
        if not isinstance(data, dict) or data.get("kind") != "render-matrix":
            continue
        if data.get("provenance", {}).get("candidate_sha") != args.candidate_sha or not data.get("complete"):
            raise ValueError(f"Cannot sample a partial or differently tagged matrix: {file}")
        for c in data["cases"]:
            renderer, style, piece, variant = c["row"].split("/")
            if (c["status"] != "matched" or c["options"]["id"] != "zh-auto-named" or
                    not c["comparison"]["pixels"]["equal"] or styles.get(style) != renderer):
                continue
            if next(f for f in data["scope"]["fixtures"] if f["id"] == c["fixture"])["kind"] != "synthetic_render_sample":
                continue
            pieces.add(piece)
            options.setdefault((style, piece), []).append((file, c, variant))
    if len(styles) != 26 or len(pieces) != 28:
        raise ValueError("The complete 26-style, 28-family review scope is unavailable")

    # Match each style to a different work where possible; then add the two
    # remaining families. This avoids mistaking 26 versions of tier for a review
    # of the artwork system. Choosing default variants is only a readability aid.
    matched = {}

    def assign(style, visited):
        for piece in sorted(pieces, key=lambda p: (sum((s, p) in options for s in styles), p)):
            if piece in visited or (style, piece) not in options:
                continue
            visited.add(piece)
            if piece not in matched or assign(matched[piece], visited):
                matched[piece] = style
                return True
        return False

    for style in styles:
        if not assign(style, set()):
            raise ValueError(f"Cannot represent style {style} with a distinct artwork family")
    for piece in sorted(pieces - matched.keys()):
        matched[piece] = next(style for style in styles if (style, piece) in options)

    records, pages = [], []
    for piece, style in sorted(matched.items(), key=lambda item: (list(styles).index(item[1]), item[0])):
        choices = options[(style, piece)]
        file, case, variant = min(choices, key=lambda row: (row[2] not in ("default", "original", "rows", "classic"), row[2]))
        img = case["candidate"]["image"]
        src = (file.parent / img["path"]).resolve()
        if not src.is_relative_to(file.parent) or sha(src.read_bytes()) != img["sha256"]:
            raise ValueError(f"Candidate image integrity failed: {src}")
        records.append({"row": case["row"], "evidence": f"{file.relative_to(root).as_posix()}#{case['key']}",
                        "image": src.relative_to(root).as_posix(), "sha256": img["sha256"],
                        "candidate_rgba_sha256": case["comparison"]["candidate_rgba_sha256"]})
    for start in range(0, len(records), 8):
        group = records[start:start + 8]
        sheet = Image.new("RGB", (728, ((len(group) + 1) // 2) * 440), "#efede8")
        draw = ImageDraw.Draw(sheet)
        for i, record in enumerate(group):
            x, y = (i % 2) * 364, (i // 2) * 440
            draw.text((x + 12, y + 10), record["row"], fill="#272522")
            with Image.open(root / record["image"]) as opened:
                im = opened.convert("RGBA")
                im.thumbnail((340, 400), Image.Resampling.LANCZOS)
                sheet.paste(im, (x + (364 - im.width) // 2, y + 30), im)
        name = f"artwork-families-{start // 8 + 1}.png"
        sheet.save(output / name)
        pages.append({"path": name, "sha256": sha((output / name).read_bytes()), "rows": [r["row"] for r in group]})
    index = {"schema_version": 1, "candidate_sha": args.candidate_sha, "status": "review_input_only",
             "scope": "One Chinese, auto-grouped, named static image per family, covering all 26 desktop styles and 28 shared families. All underlying images already match the original pixel-for-pixel. Contacts are resized for inspection; the original images remain available in each referenced case. No native or motion verdict is implied.",
             "cases": records, "pages": pages}
    (output / "review-input.json").write_text(json.dumps(index, indent=2) + "\n")
    print(json.dumps({"output": str(output), "styles": len(set(matched.values())), "families": len(records), "pages": len(pages)}))


if __name__ == "__main__":
    main()
