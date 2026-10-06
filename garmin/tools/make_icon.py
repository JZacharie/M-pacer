"""Génère l'icône du lanceur Connect IQ (garmin/resources/drawables/launcher_icon.png).

Reproductible : python garmin/tools/make_icon.py
Dépendance : Pillow (fourni par l'environnement du dépôt).
"""
from pathlib import Path

from PIL import Image, ImageDraw

SIZE = 61
OUT = Path(__file__).resolve().parents[1] / "resources" / "drawables" / "launcher_icon.png"

DARK = (13, 20, 33, 255)
CYAN = (34, 211, 238, 255)
AMBER = (251, 146, 60, 255)


def main() -> None:
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    margin = 1
    draw.ellipse((margin, margin, SIZE - 1 - margin, SIZE - 1 - margin), fill=DARK, outline=CYAN, width=2)
    # Anneau intérieur (cible d'athlétisme).
    ring = 14
    draw.ellipse((ring, ring, SIZE - 1 - ring, SIZE - 1 - ring), outline=(30, 64, 96, 255), width=2)

    # Flèche d'allure montante.
    draw.line((14, 38, 26, 26, 33, 33, 47, 19), fill=CYAN, width=4, joint="curve")
    draw.polygon([(43, 14), (51, 16), (46, 23)], fill=AMBER)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    img.save(OUT)
    print(f"{OUT} ({img.size[0]}x{img.size[1]})")


if __name__ == "__main__":
    main()
