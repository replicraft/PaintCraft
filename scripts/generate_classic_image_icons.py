#!/usr/bin/env python3
"""Original Classic ribbon vectors; theme ink retains blue accents."""
from pathlib import Path
import json

OUT = Path(__file__).resolve().parents[1] / "crates/presentation/paintcraft-ui/assets/icons/classic-image"


def path(d, accent=False):
    return f'<path d="{d}"' + (' stroke="#60cdff"' if accent else '') + '/>'


rotate = path('M5 8C6 3 15 1 19 6M15 6H19V2') + '<path d="M5 20L17 15L17 20Z" fill="#60cdff" stroke="none"/>'
icons = {
    'layers': path('M2 8L12 2L22 8L12 14ZM2 12L12 18L22 12') + path('M2 16L12 22L22 16', True),
    'crop': path('M6 2V18H22M2 6H18V22'),
    'remove-background': path('M2 7L7 2M2 12L12 2M2 17L17 2M2 22L6 18M12 22L22 12M17 22L22 17M22 7L19 10') + '<rect x="8" y="8" width="8" height="8" rx="1"/>' + path('M12 6V18M6 12H18', True),
    'resize': '<rect x="2" y="8" width="14" height="14" rx="2" stroke-dasharray="3 3"/>' + path('M3 19L7 15L11 19L14 16') + '<circle cx="11" cy="12" r="1"/>' + path('M10 3H21V14M21 3L12 12', True),
    'rotate-clockwise': rotate,
    'rotate-counterclockwise': f'<g transform="translate(24 0) scale(-1 1)">{rotate}</g>',
    'rotate-180': path('M3 10C3 2 21 2 21 10M17 7L21 10L23 6') + '<path d="M4 21L17 15L17 21Z" fill="#60cdff" stroke="none"/>',
    'rotate-custom': path('M4 9C6 2 16 2 20 7M16 7H20V3M4 20H20M7 20L17 11', True) + '<circle cx="12" cy="12" r="2"/>',
    'flip-horizontal': path('M12 2V22') + '<path d="M9 6L2 12L9 18Z"/><path d="M15 6L22 12L15 18Z" fill="#60cdff" stroke="none"/>',
    'flip-vertical': path('M2 12H22') + '<path d="M6 9L12 2L18 9Z"/><path d="M6 15L12 22L18 15Z" fill="#60cdff" stroke="none"/>',
}
OUT.mkdir(parents=True, exist_ok=True)
for theme, ink in [('light', '#181818'), ('dark', '#f5f5f5')]:
    for name, geometry in icons.items():
        (OUT / f'{name}-{theme}.svg').write_text(f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24">
<title>{name.replace('-', ' ').title()}</title>
<g fill="none" stroke="{ink}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
{geometry}
</g>
</svg>
''')
(OUT / 'manifest.json').write_text(json.dumps({
    'provenance': 'Original PaintCraft accented ribbon vectors, not extracted Windows assets.',
    'reference': 'docs/temp/paint-references/photos/image.png',
    'themes': ['light', 'dark'],
    'icons': list(icons),
}, indent=2) + '\n')
print(f'Wrote {len(icons) * 2} theme-specific Classic Image SVGs to {OUT}')
