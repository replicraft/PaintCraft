#!/usr/bin/env python3
"""Original tintable menu vectors following the supplied Classic UI references."""
from pathlib import Path
import json

OUT = Path(__file__).resolve().parents[1] / 'crates/presentation/paintcraft-ui/assets/icons/classic-menus'


def path(d):
    return f'<path d="{d}"/>'


def rect(x, y, w, h, radius=1):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{radius}"/>'


save = path('M3 3H18L21 6V21H3ZM7 3V10H17V3M7 21V14H17V21M14 5V8')
pen = 'M12 20L19 13L22 16L15 23H12Z'
save_as = ('<defs><mask id="pen-gap" maskUnits="userSpaceOnUse" x="0" y="0" width="24" height="24"><rect width="24" height="24" fill="#fff" stroke="none"/>'
           + f'<path d="{pen}" fill="#000" stroke="#000" stroke-width="4"/></mask></defs>'
           + f'<g mask="url(#pen-gap)">{save}</g>' + path(pen) + path('M18 14L21 17'))
undo = path('M3 9H13Q21 9 21 17M3 9L8 4M3 9L8 14')
glass = '<circle cx="10" cy="10" r="7"/>' + path('M15 15L22 22')
selection_cells = [(x, y) for y in [3, 10, 17] for x in [3, 10, 17]]
select_all = ''.join(rect(x, y, 4, 4, .5) for x, y in selection_cells)
invert = ''.join(f'<rect x="{x}" y="{y}" width="4" height="4" rx=".5"' + (' fill="#fff"' if i % 2 == 0 else '') + '/>' for i, (x, y) in enumerate(selection_cells))
icons = {
    'new': path('M5 2H14L20 8V22H5ZM14 2V8H20'),
    'open': path('M2 20V5H9L12 8H21V10M2 20H19L22 10H7Z'),
    'import-image': path('M11 4H21V21H3V12M3 17L9 11L15 17L18 14L21 17') + '<circle cx="17" cy="8" r="1.2"/>' + '<circle cx="5" cy="5" r="4"/>' + path('M5 2.5V7.5M3 5.5L5 7.5L7 5.5'),
    'recent': '<circle cx="12" cy="12" r="9"/>' + path('M12 5V12H17'),
    'desktop-background': rect(2, 3, 20, 15) + path('M8 22H16M12 18V22M3 14L8 9L13 14L17 10L21 14') + '<circle cx="16" cy="7" r="1.2"/>',
    'image-properties': path('M13 4H21V22H3V14M3 18L9 12L14 17L17 14L21 18') + '<circle cx="17" cy="8" r="1.2"/><circle cx="6" cy="6" r="3"/>' + path('M6 1V3M6 9V11M1 6H3M9 6H11M2.5 2.5L4 4M8 8L9.5 9.5M2.5 9.5L4 8M8 4L9.5 2.5'),
    'thumbnail': rect(6, 3, 16, 16) + path('M3 8H2V22H17V21M6 15L11 10L16 15L19 12L22 15') + '<circle cx="17" cy="7" r="1.2"/>',
    'save': save,
    'save-as': save_as,
    'print': path('M6 9V3H18V9M6 18H3V9H21V18H18M6 14H18V22H6Z') + '<circle cx="18" cy="11.5" r=".5" fill="#fff"/>',
    'share': path('M3 12V21H20V15M10 12Q11 6 17 6H20M15 2L20 6L15 10'),
    'canvas-size': rect(3, 4, 18, 15) + path('M5 15L10 10L14 14L17 11L20 15') + '<circle cx="17" cy="8" r="1"/>',
    'close': path('M4 4L20 20M20 4L4 20'),
    'cut': '<circle cx="7" cy="17" r="4"/><circle cx="17" cy="17" r="4"/>' + path('M5 3L17 17M19 3L7 17'),
    'copy': rect(7, 3, 13, 15) + path('M4 7H2V22H15V20'),
    'paste': rect(5, 5, 15, 17) + rect(8, 2, 8, 5),
    'copy-merged': rect(3, 7, 12, 15) + path('M7 4H19V18M11 1H23V14'),
    'undo': undo,
    'redo': f'<g transform="translate(24 0) scale(-1 1)">{undo}</g>',
    'select-all': select_all,
    'invert-selection': invert,
    'lasso-select': '<path d="M6 3C1 5 2 9 3 13S3 21 9 21S21 20 21 14S20 2 14 3S9 1 6 3Z" stroke-dasharray="3 2"/>',
    'delete-layer': path('M3 5H21M8 5V2H16V5M5 5L7 22H17L19 5M10 9V18M14 9V18'),
    'rectangle-select': '<rect x="3" y="4" width="18" height="16" rx="1" stroke-dasharray="3 3"/>',
    'clear-canvas': path('M10 12L18 2M6 10L14 14L10 23L1 18ZM3 15L11 19M5 12L13 16M2 19L10 23'),
    'zoom-in': glass + path('M6 10H14M10 6V14'),
    'zoom-out': glass + path('M6 10H14'),
    'fit-window': path('M3 9V3H9M15 3H21V9M21 15V21H15M9 21H3V15M7 7L11 11M17 7L13 11M7 17L11 13M17 17L13 13'),
    'actual-size': glass + path('M8 8L11 6V13M8 13H14'),
    'pixel-grid': rect(3, 3, 18, 18) + path('M9 3V21M15 3V21M3 9H21M3 15H21'),
    'rulers': path('M2 2H7V17H22V22H2ZM4 6H7M4 10H7M4 14H7M11 17V20M15 17V20M19 17V20'),
    'resize': path('M15 3H21V9M21 3L14 10M3 15V21H9M3 21L10 14'),
    'brush': path('M6 2V10C6 16 8 20 12 22C16 20 18 16 18 10V2M4 5H20M6 9H18M9 12V17M12 12V19M15 12V17'),
    'pencil': path('M14 4H3V21H20V12M8 15L18 5L21 8L11 18H8ZM17 6L20 9'),
    'bucket': path('M4 10L11 3L19 11L12 18ZM8 6L15 13M10 2L21 13M4 14H2V22H16V20M20 15C24 19 24 22 20 22C16 22 16 19 20 15Z'),
}
OUT.mkdir(parents=True, exist_ok=True)
for name, geometry in icons.items():
    (OUT / f'{name}.svg').write_text(f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24">
<title>{name.replace('-', ' ').title()}</title>
<g fill="none" stroke="#fff" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
{geometry}
</g>
</svg>
''')
(OUT / 'manifest.json').write_text(json.dumps({
    'provenance': 'Original PaintCraft outline menu vectors; not extracted native assets.',
    'references': ['docs/temp/paint-references/photos/im1age.png', 'docs/temp/paint-references/photos/imag1e.png', 'docs/temp/paint-references/photos/ima2ge.png', 'docs/temp/paint-references/photos/image.png', 'docs/temp/paint-references/photos/i11mage.png', 'docs/temp/paint-references/photos/ima1111ge.png'],
    'icons': [{'name': name, 'file': f'{name}.svg'} for name in icons],
}, indent=2) + '\n')
print(f'Wrote {len(icons)} Classic menu SVGs to {OUT}')
