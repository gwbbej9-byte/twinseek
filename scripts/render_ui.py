"""Draw a dark 920x520 still of the twinseek ratatui layout."""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

OUT = Path(__file__).resolve().parents[1] / "screenshots" / "ui.png"
W, H = 920, 520

BG = (13, 17, 23)
PANEL = (22, 27, 34)
BORDER = (88, 166, 255)
BORDER_DIM = (48, 54, 61)
TITLE = (63, 185, 80)
TEXT = (201, 209, 216)
MUTED = (139, 148, 158)
WASTE = (210, 153, 34)
SEL = (31, 111, 235)
SEL_FG = (255, 255, 255)
MARK = (248, 81, 73)
FOOT = (22, 27, 34)


def font(size: int) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    for path in (
        r"C:\Windows\Fonts\cascadiamono.ttf",
        r"C:\Windows\Fonts\CascadiaMono.ttf",
        r"C:\Windows\Fonts\consola.ttf",
        r"C:\Windows\Fonts\lucon.ttf",
    ):
        p = Path(path)
        if p.exists():
            return ImageFont.truetype(str(p), size)
    return ImageFont.load_default()


def draw_box(d: ImageDraw.ImageDraw, xy: tuple[int, int, int, int], color: tuple[int, int, int]) -> None:
    x0, y0, x1, y1 = xy
    d.rounded_rectangle(xy, radius=6, outline=color, width=2)
    # title gap is drawn by caller


def main() -> None:
    OUT.parent.mkdir(parents=True, exist_ok=True)
    img = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(img)
    f16 = font(16)
    f14 = font(14)
    f18 = font(18)
    f13 = font(13)

    d.text((20, 14), "twinseek", font=f18, fill=TITLE)
    d.text((128, 16), "testdata", font=f16, fill=TEXT)
    d.text((250, 16), "·  3 groups  ·  342 B wasted", font=f14, fill=MUTED)

    left = (16, 48, 448, 468)
    right = (460, 48, 904, 468)
    d.rounded_rectangle(left, radius=8, fill=PANEL, outline=BORDER, width=2)
    d.rounded_rectangle(right, radius=8, fill=PANEL, outline=BORDER_DIM, width=2)

    d.rectangle((28, 40, 118, 58), fill=BG)
    d.text((34, 40), " groups ● ", font=f13, fill=BORDER)
    d.rectangle((472, 40, 560, 58), fill=BG)
    d.text((478, 40), " files ", font=f13, fill=BORDER_DIM)

    rows = [
        (True, " 3× ", "    78 B wasted  ", "notes.txt"),
        (False, " 2× ", "   200 B wasted  ", "shot.dat"),
        (False, " 2× ", "    64 B wasted  ", "left.bin"),
    ]
    y = 72
    for selected, count, waste, name in rows:
        if selected:
            d.rounded_rectangle((28, y - 4, 436, y + 26), radius=4, fill=SEL)
            cfill, wfill, nfill = SEL_FG, SEL_FG, SEL_FG
            prefix = "▶"
        else:
            cfill, wfill, nfill = TITLE, WASTE, TEXT
            prefix = " "
        d.text((36, y), prefix, font=f16, fill=cfill)
        d.text((54, y), count, font=f16, fill=cfill)
        d.text((100, y), waste, font=f16, fill=wfill)
        d.text((278, y), name, font=f16, fill=nfill)
        y += 36

    files = [
        (False, "  archive/notes.txt"),
        (False, "  notes.copy.txt"),
        (True, "  notes.txt"),
    ]
    y = 72
    for selected, path in files:
        if selected:
            d.rounded_rectangle((472, y - 4, 892, y + 26), radius=4, fill=SEL)
            fill = SEL_FG
            prefix = "▶"
        else:
            fill = TEXT
            prefix = " "
        d.text((480, y), prefix, font=f16, fill=fill)
        d.text((500, y), path, font=f16, fill=fill)
        y += 36

    d.rounded_rectangle((16, 478, 904, 508), radius=6, fill=FOOT, outline=BORDER_DIM, width=1)
    keys = [
        (" j/k ", TITLE, "move"),
        (" enter ", TITLE, "expand"),
        (" d ", TITLE, "mark"),
        (" q ", TITLE, "quit"),
    ]
    x = 24
    for key, kfill, label in keys:
        d.text((x, 484), key, font=f14, fill=kfill)
        x += int(d.textlength(key, font=f14))
        d.text((x, 484), label, font=f14, fill=MUTED)
        x += int(d.textlength(label, font=f14)) + 18
    d.text((620, 484), "marked 0  ·  never deletes", font=f14, fill=WASTE)

    img.save(OUT, "PNG")
    print(f"wrote {OUT} {img.size[0]}x{img.size[1]}")


if __name__ == "__main__":
    main()
