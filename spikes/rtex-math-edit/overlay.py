"""Draw host-only placeholder outlines over a full TeX page; never edit the PDF."""
import argparse
import json
import subprocess
from pathlib import Path
from PIL import Image, ImageDraw
from geometry2 import materialize, merge_raw
from round2 import document


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pdf', required=True, type=Path)
    parser.add_argument('--pages', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--font', required=True)
    args = parser.parse_args()
    subprocess.run(['pdftoppm','-r','150','-png','-singlefile',str(args.pdf),str(args.out.with_suffix(''))],
                   check=True, capture_output=True)
    image = Image.open(args.out).convert('RGB')
    draw = ImageDraw.Draw(image)
    scale = 150 / 72.27 / 65536
    raw = merge_raw(json.loads(args.pages.read_text()))
    for unit in document(args.font).units:
        for slot in materialize(unit, raw)['slots']:
            box = [v*scale for v in slot['bbox']]
            draw.rectangle(box, outline='#11884b', width=2)
    image.save(args.out)


if __name__ == '__main__': main()
