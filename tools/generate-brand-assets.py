"""Generate crisp Swaggen official brand icons (.ico, .png) and Windows Installer BMPs."""
import math
import os
from PIL import Image, ImageDraw, ImageFont

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
TEAL = (0x24, 0x5C, 0x59, 255)
SALMON = (0xF5, 0xA8, 0x9A, 255)
INK = (0x20, 0x21, 0x24, 255)
MUTED = (0x65, 0x68, 0x6F, 255)
BORDER = (0xE5, 0xE7, 0xEB, 255)
WHITE = (255, 255, 255, 255)


def cubic_bezier(p0, p1, p2, p3, steps=40):
    pts = []
    for i in range(steps + 1):
        t = i / steps
        mt = 1.0 - t
        x = mt**3 * p0[0] + 3 * mt**2 * t * p1[0] + 3 * mt * t**2 * p2[0] + t**3 * p3[0]
        y = mt**3 * p0[1] + 3 * mt**2 * t * p1[1] + 3 * mt * t**2 * p2[1] + t**3 * p3[1]
        pts.append((x, y))
    return pts


def draw_swaggen_mark(size: int, with_tile: bool = True) -> Image.Image:
    """Render official Figma Swaggen Brand/Mark (4:81) with 4x supersampling."""
    ss = 4
    w = size * ss
    img = Image.new("RGBA", (w, w), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    if with_tile:
        pad = int(w * 0.04)
        rad = int(w * 0.22)
        draw.rounded_rectangle(
            [pad, pad, w - pad, w - pad],
            radius=rad,
            fill=WHITE,
            outline=BORDER,
            width=max(2, int(w * 0.014)),
        )

    # Map official 32x32 Figma brand-mark coordinates into centered canvas
    scale = (w * (0.78 if with_tile else 0.90)) / 24.0
    ox = (w - 22.0 * scale) / 2.0
    oy = (w - 20.5 * scale) / 2.0

    def pt(x, y):
        return (ox + x * scale, oy + y * scale)

    # Upper code bracket (SVG 00: M14 0 H6.75 C2.5 0 0 2.5 0 6.25 C0 10 2.5 12.25 6.75 12.25 H9.25 L12.75 8.25 H6.75 C5 8.25 4 7.5 4 6.25 C4 4.75 5 4 6.75 4 H14 C14.825 4 15.5 3.325 15.5 2.5 V1.5 C15.5 0.675 14.825 0 14 0 Z)
    ux, uy = 0.0, 0.0
    upper = []
    upper.append(pt(ux + 14.0, uy + 0.0))
    upper.append(pt(ux + 6.75, uy + 0.0))
    upper.extend(cubic_bezier(pt(ux + 6.75, uy + 0.0), pt(ux + 2.5, uy + 0.0), pt(ux + 0.0, uy + 2.5), pt(ux + 0.0, uy + 6.25)))
    upper.extend(cubic_bezier(pt(ux + 0.0, uy + 6.25), pt(ux + 0.0, uy + 10.0), pt(ux + 2.5, uy + 12.25), pt(ux + 6.75, uy + 12.25)))
    upper.append(pt(ux + 9.25, uy + 12.25))
    upper.append(pt(ux + 12.75, uy + 8.25))
    upper.append(pt(ux + 6.75, uy + 8.25))
    upper.extend(cubic_bezier(pt(ux + 6.75, uy + 8.25), pt(ux + 5.0, uy + 8.25), pt(ux + 4.0, uy + 7.5), pt(ux + 4.0, uy + 6.25)))
    upper.extend(cubic_bezier(pt(ux + 4.0, uy + 6.25), pt(ux + 4.0, uy + 4.75), pt(ux + 5.0, uy + 4.0), pt(ux + 6.75, uy + 4.0)))
    upper.append(pt(ux + 14.0, uy + 4.0))
    upper.extend(cubic_bezier(pt(ux + 14.0, uy + 4.0), pt(ux + 14.825, uy + 4.0), pt(ux + 15.5, uy + 3.325), pt(ux + 15.5, uy + 2.5)))
    upper.append(pt(ux + 15.5, uy + 1.5))
    upper.extend(cubic_bezier(pt(ux + 15.5, uy + 1.5), pt(ux + 15.5, uy + 0.675), pt(ux + 14.825, uy + 0.0), pt(ux + 14.0, uy + 0.0)))
    draw.polygon(upper, fill=TEAL)

    # Lower code bracket (SVG 01, offset (+2.5, +8.25))
    lx, ly = 2.5, 8.25
    lower = []
    lower.append(pt(lx + 12.75, ly + 0.0))
    lower.append(pt(lx + 9.25, ly + 4.0))
    lower.append(pt(lx + 12.25, ly + 4.0))
    lower.extend(cubic_bezier(pt(lx + 12.25, ly + 4.0), pt(lx + 14.0, ly + 4.0), pt(lx + 15.0, ly + 4.75), pt(lx + 15.0, ly + 6.0)))
    lower.extend(cubic_bezier(pt(lx + 15.0, ly + 6.0), pt(lx + 15.0, ly + 7.5), pt(lx + 14.0, ly + 8.25), pt(lx + 12.25, ly + 8.25)))
    lower.append(pt(lx + 1.5, ly + 8.25))
    lower.extend(cubic_bezier(pt(lx + 1.5, ly + 8.25), pt(lx + 0.675, ly + 8.25), pt(lx + 0.0, ly + 8.925), pt(lx + 0.0, ly + 9.75)))
    lower.append(pt(lx + 0.0, ly + 10.75))
    lower.extend(cubic_bezier(pt(lx + 0.0, ly + 10.75), pt(lx + 0.0, ly + 11.575), pt(lx + 0.675, ly + 12.25), pt(lx + 1.5, ly + 12.25)))
    lower.append(pt(lx + 12.25, ly + 12.25))
    lower.extend(cubic_bezier(pt(lx + 12.25, ly + 12.25), pt(lx + 16.5, ly + 12.25), pt(lx + 19.0, ly + 9.75), pt(lx + 19.0, ly + 6.0)))
    lower.extend(cubic_bezier(pt(lx + 19.0, ly + 6.0), pt(lx + 19.0, ly + 2.25), pt(lx + 16.5, ly + 0.0), pt(lx + 12.75, ly + 0.0)))
    draw.polygon(lower, fill=TEAL)

    # Salmon square accent dot (SVG 02: 3x3 at top-right of upper bracket)
    sx, sy = 18.0, 0.5
    draw.rectangle([pt(sx, sy), pt(sx + 3.2, sy + 3.2)], fill=SALMON)

    return img.resize((size, size), Image.Resampling.LANCZOS)


def load_font(size: int, bold: bool = False):
    candidates = []
    local_fonts = os.path.join(os.environ.get("LOCALAPPDATA", ""), "Microsoft", "Windows", "Fonts")
    if bold:
        candidates.extend([
            os.path.join(local_fonts, "JetBrainsMono-Bold.ttf"),
            r"C:\Windows\Fonts\segoeuib.ttf",
            r"C:\Windows\Fonts\arialbd.ttf",
        ])
    else:
        candidates.extend([
            os.path.join(local_fonts, "JetBrainsMono-Regular.ttf"),
            r"C:\Windows\Fonts\segoeui.ttf",
            r"C:\Windows\Fonts\arial.ttf",
        ])
    for p in candidates:
        if os.path.isfile(p):
            return ImageFont.truetype(p, size)
    return ImageFont.load_default()


def main():
    assets_dir = os.path.join(REPO_ROOT, "assets")
    inst_assets = os.path.join(REPO_ROOT, "installer", "assets")
    os.makedirs(assets_dir, exist_ok=True)
    os.makedirs(inst_assets, exist_ok=True)

    # 1. App Icon PNG (256x256) + Multi-resolution Windows ICO
    icon_256 = draw_swaggen_mark(256, with_tile=True)
    icon_png_path = os.path.join(assets_dir, "swaggen-icon-256.png")
    icon_256.save(icon_png_path, format="PNG")

    ico_path = os.path.join(assets_dir, "swaggen.ico")
    icon_256.save(
        ico_path,
        format="ICO",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )

    # 2. Inno Setup Wizard Banner (164 x 314, 24-bit RGB BMP)
    banner = Image.new("RGB", (164, 314), (255, 255, 255))
    bdraw = ImageDraw.Draw(banner)
    # Clean teal accent bar at top + crisp right divider
    bdraw.rectangle([0, 0, 164, 8], fill=TEAL[:3])
    bdraw.rectangle([163, 0, 164, 314], fill=BORDER[:3])
    mark_72 = draw_swaggen_mark(72, with_tile=False)
    banner.paste(mark_72, (46, 56), mark_72)
    f_title = load_font(20, bold=True)
    f_sub = load_font(11, bold=False)
    f_small = load_font(10, bold=False)
    bdraw.text((34, 146), "swaggen", fill=INK[:3], font=f_title)
    bdraw.text((22, 180), "Native Windows", fill=MUTED[:3], font=f_sub)
    bdraw.text((22, 198), "API Studio", fill=MUTED[:3], font=f_sub)
    bdraw.line([(22, 236), (140, 236)], fill=BORDER[:3], width=1)
    bdraw.text((22, 252), "Design  Test", fill=TEAL[:3], font=f_small)
    bdraw.text((22, 270), "Collaborate", fill=TEAL[:3], font=f_small)
    banner.save(os.path.join(inst_assets, "wizard-banner.bmp"), format="BMP")

    # 3. Inno Setup Small Top-Right Icon (55 x 58, 24-bit RGB BMP)
    small = Image.new("RGB", (55, 58), (255, 255, 255))
    mark_44 = draw_swaggen_mark(44, with_tile=False)
    small.paste(mark_44, (5, 7), mark_44)
    small.save(os.path.join(inst_assets, "wizard-small.bmp"), format="BMP")

    # 4. WiX MSI Banner (493 x 58, 24-bit RGB BMP)
    msi_ban = Image.new("RGB", (493, 58), (255, 255, 255))
    mbdraw = ImageDraw.Draw(msi_ban)
    mbdraw.line([(0, 57), (493, 57)], fill=BORDER[:3], width=1)
    msi_ban.paste(mark_44, (438, 7), mark_44)
    msi_ban.save(os.path.join(inst_assets, "msi-banner.bmp"), format="BMP")

    # 5. WiX MSI Dialog Background (493 x 312, 24-bit RGB BMP)
    msi_dlg = Image.new("RGB", (493, 312), (255, 255, 255))
    msi_dlg.paste(banner.crop((0, 0, 164, 312)), (0, 0))
    msi_dlg.save(os.path.join(inst_assets, "msi-dialog.bmp"), format="BMP")

    print("Generated all Swaggen brand assets cleanly:")
    for p in [
        icon_png_path,
        ico_path,
        os.path.join(inst_assets, "wizard-banner.bmp"),
        os.path.join(inst_assets, "wizard-small.bmp"),
        os.path.join(inst_assets, "msi-banner.bmp"),
        os.path.join(inst_assets, "msi-dialog.bmp"),
    ]:
        print(f"  {p} ({os.path.getsize(p)} bytes)")


if __name__ == "__main__":
    main()
