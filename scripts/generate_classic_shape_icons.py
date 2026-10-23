#!/usr/bin/env python3
"""Original monochrome Classic gallery vectors, following the supplied UI references."""
from pathlib import Path
import json
import math

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'crates/presentation/paintcraft-ui/assets/icons/classic-shapes'


def path(d):
    return f'<path d="{d}"/>'


def star(points, inner):
    vertices = []
    for i in range(points * 2):
        angle = -math.pi / 2 + i * math.pi / points
        radius = 10 if i % 2 == 0 else inner
        vertices.append(f'{12 + math.cos(angle) * radius:.3f} {12 + math.sin(angle) * radius:.3f}')
    return path('M' + 'L'.join(vertices) + 'Z')


arrow = path('M2 8H13V3L22 12L13 21V16H2Z')
icons = {
    'line': path('M3 3L21 21'),
    'curve': path('M2 17C5 4 7 4 9 13S13 22 16 10S20 3 22 6'),
    'ellipse': '<circle cx="12" cy="12" r="9"/>',
    'rectangle': '<rect x="3" y="3" width="18" height="18"/>',
    'rounded-rectangle': '<rect x="3" y="3" width="18" height="18" rx="4"/>',
    'polygon': path('M8 3H14L10 13H18L15 21H2Z'),
    'triangle': path('M12 3L22 21H2Z'),
    'right-triangle': path('M3 3V21H21Z'),
    'diamond': path('M12 2L22 12L12 22L2 12Z'),
    'pentagon': path('M12 2L22 9L18 22H6L2 9Z'),
    'hexagon': path('M12 2L21 7V17L12 22L3 17V7Z'),
    'right-arrow': arrow,
    'left-arrow': f'<g transform="rotate(180 12 12)">{arrow}</g>',
    'up-arrow': f'<g transform="rotate(-90 12 12)">{arrow}</g>',
    'down-arrow': f'<g transform="rotate(90 12 12)">{arrow}</g>',
    'four-point-star': star(4, 3),
    'star': star(5, 4.5),
    'six-point-star': star(6, 5.5),
    'rounded-speech-bubble': path('M5 3H19Q22 3 22 6V14Q22 17 19 17H12L6 22V17H5Q2 17 2 14V6Q2 3 5 3Z'),
    'speech-bubble': path('M12 3C25 3 25 17 12 17L6 22V16C-1 12 0 3 12 3Z'),
    'cloud-callout': path('M5 5Q7 1 11 4Q15 0 18 5Q24 4 21 10Q25 14 20 16Q20 21 15 19Q12 23 9 19L4 22L6 17Q0 17 3 12Q-1 8 5 5Z'),
    'heart': path('M12 21L3 12C-2 6 6 -1 12 6C18 -1 26 6 21 12Z'),
    'lightning': path('M13 2L4 13H11L9 22L21 10H14Z'),
}

OUT.mkdir(parents=True, exist_ok=True)
manifest = []
for name, geometry in icons.items():
    label = name.replace('-', ' ').title()
    svg = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24">
<title>{label}</title>
<g fill="none" stroke="#fff" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
{geometry}
</g>
</svg>
'''
    (OUT / f'{name}.svg').write_text(svg)
    manifest.append({'name': name, 'file': f'{name}.svg'})
(OUT / 'manifest.json').write_text(json.dumps({
    'provenance': 'Original PaintCraft vectors for the monochrome Classic gallery; not extracted native assets.',
    'references': ['docs/temp/paint-references/photos/image.png', 'docs/temp/paint-references/photos/im2age.png'],
    'icons': manifest,
}, indent=2) + '\n')
print(f'Wrote {len(icons)} original Classic shape SVGs to {OUT}')
