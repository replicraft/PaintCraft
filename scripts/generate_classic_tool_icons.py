#!/usr/bin/env python3
"""Original tintable Classic v11 Tools artwork; independent of colored icons."""
from pathlib import Path
import json

OUT = Path(__file__).resolve().parents[1] / 'crates/presentation/paintcraft-ui/assets/icons/classic-tools'
icons = {
    'pencil': '<path d="M4 16L16 4L20 8L8 20L3 21ZM14 6L18 10M4 16L8 20M16 4L18 2Q19 1 20 2L22 4Q23 5 22 6L20 8"/>',
    'fill': '<path d="M4 10L11 3L19 11L12 18ZM8 6L15 13M10 2L21 13M4 10H18M20 15C24 19 24 22 20 22C16 22 16 19 20 15Z"/>',
    'text': '<path d="M3 21L12 2L21 21M6 15H18M1 21H6M18 21H23"/>',
    'eraser': '<path d="M3 13L14 2Q15 1 16 2L22 8Q23 9 22 10L11 21H9L3 15Q2 14 3 13ZM8 8L17 17M11 21H22"/>',
    'eyedropper': '<path d="M6 12L3 20L4 21L12 18L18 12M6 12L12 18M5 10L14 19M13 7L18 2Q19 1 20 2L22 4Q23 5 22 6L17 11M11 5L19 13"/>',
    'zoom-in': '<circle cx="10" cy="10" r="7"/><path d="M15 15L22 22M6 10H14M10 6V14"/>',
}
OUT.mkdir(parents=True, exist_ok=True)
for name, geometry in icons.items():
    (OUT / f'{name}.svg').write_text(f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24">
<title>{name.replace('-', ' ').title()}</title>
<g fill="none" stroke="#fff" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">{geometry}</g>
</svg>
''')
(OUT / 'manifest.json').write_text(json.dumps({
    'provenance': 'Original PaintCraft outline tool vectors; not extracted native assets.',
    'references': ['docs/temp/paint-references/photos/image.png', 'docs/temp/paint-references/photos/mspaint-text-settings.png'],
    'icons': [{'name': name, 'file': f'{name}.svg'} for name in icons],
}, indent=2) + '\n')
print(f'Wrote {len(icons)} Classic tool SVGs')
