#!/usr/bin/env python3
"""Original Classic status vectors based on the supplied full-window reference."""
from pathlib import Path
import json

OUT = Path(__file__).resolve().parents[1] / 'crates/presentation/paintcraft-ui/assets/icons/classic-status'
glass = '<circle cx="10" cy="10" r="7"/><path d="M15 15L22 22"/>'
icons = {
    'pointer': '<path d="M4 2V21L9 16L13 23L16 21L12 14H19Z"/>',
    'selection': '<rect x="3" y="4" width="18" height="16" rx="1" stroke-dasharray="3 3"/>',
    'canvas-size': '<path d="M5 2V22M2 5H22M19 2V22M2 19H22"/>',
    'fit-window': '<path d="M2 8V2H8M16 2H22V8M22 16V22H16M8 22H2V16"/><circle cx="12" cy="12" r="6"/>',
    'zoom-in': glass + '<path d="M6 10H14M10 6V14" stroke="#60cdff"/>',
    'zoom-out': glass + '<path d="M6 10H14" stroke="#60cdff"/>',
}
OUT.mkdir(parents=True, exist_ok=True)
for theme, ink in [('light', '#181818'), ('dark', '#f5f5f5')]:
    for name, geometry in icons.items():
        (OUT / f'{name}-{theme}.svg').write_text(f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24">
<title>{name.replace('-', ' ').title()}</title>
<g fill="none" stroke="{ink}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">{geometry}</g>
</svg>
''')
(OUT / 'manifest.json').write_text(json.dumps({
    'provenance': 'Original PaintCraft Classic status vectors; not extracted native assets.',
    'reference': 'docs/temp/paint-references/photos/image.png',
    'themes': ['light', 'dark'],
    'icons': list(icons),
}, indent=2) + '\n')
print(f'Wrote {len(icons) * 2} Classic status SVGs')
