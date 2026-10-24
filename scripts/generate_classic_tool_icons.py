#!/usr/bin/env python3
"""Original Classic v11 Tools vectors with themed ink and reference color accents."""
from pathlib import Path
import json

OUT = Path(__file__).resolve().parents[1] / 'crates/presentation/paintcraft-ui/assets/icons/classic-tools'
icons = {
    'pencil': '<path d="M6 15L16 5L20 9L10 19Z" fill="#f2d37f"/><path d="M3 21L6 15L10 19Z" fill="#e8d5b1"/><path d="M3 21L4.6 17.9L6.1 19.5Z" fill="INK" stroke="none"/><path d="M16 5L18 3Q19 2 20 3L22 5Q23 6 22 7L20 9Z" fill="#99a4ad"/>',
    'fill': '<path d="M4 10L11 3L19 11L12 18Z"/><path d="M4 10L19 11L12 18Z" fill="#55bce9"/><path d="M8 6L15 13M10 2L21 13"/><path d="M20 15C24 19 24 22 20 22C16 22 16 19 20 15Z" fill="#f28b64"/>',
    'text': '<path d="M3 21L12 2L21 21M6 15H18M1 21H6M18 21H23"/>',
    'eraser': '<path d="M3 13L8 8L17 17L11 21H9L3 15Q2 14 3 13Z" fill="#f5f5f5"/><path d="M8 8L14 2Q15 1 16 2L22 8Q23 9 22 10L17 17Z" fill="#f6abb4"/><path d="M11 21H22"/>',
    'eyedropper': '<path d="M4 18L7 15L10 18L4 21Z" fill="#f2d37f" stroke="none"/><path d="M6 12L3 20L4 21L12 18L18 12M6 12L12 18M5 10L14 19"/><path d="M13 7L18 2Q19 1 20 2L22 4Q23 5 22 6L17 11Z" fill="#55bce9"/><path d="M11 5L19 13"/>',
    'zoom-in': '<circle cx="10" cy="10" r="7"/><path d="M15 15L22 22"/><path d="M6 10H14M10 6V14" stroke="#55bce9"/>',
}
OUT.mkdir(parents=True, exist_ok=True)
for name in icons:
    (OUT / f'{name}.svg').unlink(missing_ok=True)
for theme, ink in [('light', '#181818'), ('dark', '#f5f5f5')]:
    for name, geometry in icons.items():
        (OUT / f'{name}-{theme}.svg').write_text(f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24">
<title>{name.replace('-', ' ').title()}</title>
<g fill="none" stroke="{ink}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">{geometry.replace('INK', ink)}</g>
</svg>
''')
(OUT / 'manifest.json').write_text(json.dumps({
    'provenance': 'Original PaintCraft accented tool vectors with themed ink; not extracted native assets.',
    'references': ['docs/temp/paint-references/photos/image.png', 'docs/temp/paint-references/photos/mspaint-text-settings.png'],
    'themes': ['light', 'dark'],
    'icons': [{'name': name, 'files': {theme: f'{name}-{theme}.svg' for theme in ['light', 'dark']}} for name in icons],
}, indent=2) + '\n')
print(f'Wrote {len(icons) * 2} theme-specific Classic tool SVGs')
