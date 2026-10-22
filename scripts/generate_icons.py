#!/usr/bin/env python3
"""Reproduce PaintCraft's hand-authored vector adaptations of the supplied icon sheet."""
from pathlib import Path
import json
import math
import colorsys

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'crates/presentation/paintcraft-ui/assets/icons'
DEFS = '''<defs>
<linearGradient id="blue" x2="0" y2="1"><stop stop-color="#59ceff"/><stop offset=".48" stop-color="#128ff5"/><stop offset="1" stop-color="#075eb9"/></linearGradient>
<linearGradient id="gold" x2="0" y2="1"><stop stop-color="#fff49c"/><stop offset=".45" stop-color="#ffd345"/><stop offset="1" stop-color="#e89308"/></linearGradient>
<linearGradient id="paper" x2="0" y2="1"><stop stop-color="#fff"/><stop offset="1" stop-color="#cbd8e8"/></linearGradient>
<linearGradient id="slate" x2="0" y2="1"><stop stop-color="#747f8f"/><stop offset="1" stop-color="#242e3c"/></linearGradient>
<linearGradient id="red" x2="0" y2="1"><stop stop-color="#ff7a95"/><stop offset="1" stop-color="#e7274e"/></linearGradient>
<linearGradient id="green" x2="0" y2="1"><stop stop-color="#b2f795"/><stop offset="1" stop-color="#27942e"/></linearGradient>
</defs>'''

def path(d, fill='url(#blue)', extra=''):
    return f'<path d="{d}" fill="{fill}" {extra}/>'

def rect(x, y, w, h, fill='url(#blue)', r=3, extra=''):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}" {extra}/>'

def circle(x, y, r, fill='url(#blue)', extra=''):
    return f'<circle cx="{x}" cy="{y}" r="{r}" fill="{fill}" {extra}/>'

def line(d, color='#fff', width=3, extra=''):
    return path(d, 'none', f'stroke="{color}" stroke-width="{width}" {extra}')

def group(body, transform):
    return f'<g transform="{transform}">{body}</g>'

paper = path('M15 6H39L52 19V57H15Z', 'url(#paper)') + path('M39 6V20H52', '#1aaaff', 'stroke-width="2"')
pencil = path('M10 46L43 9Q46 6 49 9L56 16Q58 18 55 21L22 56L7 60Z', 'url(#gold)') + path('M7 60L11 46L22 56Z', '#efcf9c') + path('M7 60L9 53L14 58Z', '#263342') + path('M43 9L49 9L56 16L51 22L39 15Z', 'url(#red)') + line('M17 44L42 18', '#fff8b9', 2)
brush = path('M25 36L45 8Q49 3 54 7Q58 10 54 16L34 43Z') + path('M25 35L35 42L29 49L18 42Z', 'url(#paper)') + path('M18 42Q8 43 8 54Q7 59 3 60Q26 64 30 49Z', 'url(#slate)') + line('M32 30L47 10', '#9ee6ff', 2)
floppy = path('M10 6H46L56 16V58H10Z') + rect(20, 6, 23, 20, 'url(#paper)', 0) + rect(33, 7, 6, 16, '#718396', 0) + rect(19, 36, 29, 21, 'url(#paper)', 1) + line('M24 43H43M24 49H43', '#96a8bc', 2)
arrow = path('M31 9L7 29L31 46V34Q48 31 49 52Q66 22 31 22Z') + line('M29 17L17 28L29 36', '#8de2ff', 2)
layer = path('M6 26L31 10L58 26L32 43Z', 'url(#paper)') + path('M8 37L32 53L56 37M8 46L32 61L56 46', 'none', 'stroke="#5b738e" stroke-width="5"')

def badge(sign, fill):
    return circle(48, 48, 13, fill) + line('M40 48H56' + ('M48 40V56' if sign == '+' else ''), '#fff', 4)

def magnifier(symbol):
    return path('M43 40L59 55Q62 59 57 62L39 45Z', 'url(#paper)') + circle(27, 27, 22, 'url(#paper)') + circle(27, 27, 16) + line(symbol, '#fff', 4)

def wheel():
    return ''.join(path(d, c, 'stroke-width="1.5"') for d, c in [
        ('M32 32V5A27 27 0 0 1 55 45Z','#f53065'),
        ('M32 32L55 45A27 27 0 0 1 9 45Z','#54c83d'),
        ('M32 32L9 45A27 27 0 0 1 32 5Z','#2e9dff')]) + circle(32, 32, 10, '#f9e77c', 'stroke="none"')

icons = []
def add(name, body):
    icons.append((name, body))

# Row 1: file and clipboard operations.
add('new', paper)
add('open', path('M5 18V55H55V18H29L23 11H7Z','url(#gold)') + path('M7 29H60L53 56H4Z','url(#gold)') + line('M10 33H54','#fff1a2',2))
add('save', floppy)
add('save-as', group(floppy,'translate(-2 -3) scale(.88)') + group(pencil,'translate(27 29) scale(.55)'))
add('print',rect(17,4,31,20,'url(#paper)')+rect(4,20,56,29,'url(#slate)',7)+rect(17,38,31,23,'url(#paper)',0)+line('M23 47H42M23 54H42','#5597c8',3)+circle(50,29,2,'#46bfff','stroke="none"'))
add('close',rect(6,6,52,52,'url(#red)',8)+line('M20 20L44 44M44 20L20 44','#fff',6))
add('undo',arrow)
add('redo',group(arrow.replace('url(#blue)','url(#slate)'), 'translate(64 0) scale(-1 1)'))
add('cut',path('M28 37L11 5Q9 1 10 12L27 45M35 37L53 4Q56 1 54 12L36 46','url(#paper)')+circle(18,48,12)+circle(46,48,12)+circle(18,48,5,'#1f3041')+circle(46,48,5,'#1f3041')+circle(32,35,3,'#d3e7ff'))
add('copy',group(paper,'translate(-8 5) scale(.9)')+group(paper,'translate(7 -2) scale(.9)'))
add('paste',rect(12,10,41,50,'url(#gold)',4)+rect(18,17,29,36,'url(#paper)',1)+rect(23,5,19,14,'url(#gold)',2)+line('M23 27H42M23 34H42M23 41H42','#a8b4c2',2))
add('copy-merged',group(paper,'translate(9 -3) scale(.9)')+group(paper,'translate(-9 7) scale(.9)'))
# Row 2: viewport and selection.
add('zoom-in',magnifier('M17 27H37M27 17V37'))
add('zoom-out',magnifier('M17 27H37'))
add('fit-window',group(magnifier('M17 27H37'),'translate(5 7) scale(.8)')+line('M5 17V5H18M46 5H59V18M5 46V59H18M46 59H59V46','#139cf5',4))
add('actual-size',magnifier('M16 23L20 20V34M17 34H23M27 24V25M27 30V31M31 23L35 20V34M32 34H38'))
add('pixel-grid',rect(7,7,50,50,'url(#slate)',1)+path('M10 10H54V54H10Z','url(#blue)','stroke="none"')+line('M24 9V55M40 9V55M9 24H55M9 40H55','#bddfff',3))
add('rulers',rect(20,4,26,56,'url(#gold)',1)+line('M22 12H35M22 20H30M22 28H35M22 36H30M22 44H35M22 52H30','#4a3823',3))
add('rectangle-select',rect(7,11,50,44,'none',2,'stroke="#138de8" stroke-dasharray="6 4" stroke-width="4"'))
add('lasso-select',path('M16 51Q4 38 11 23Q18 8 36 10Q57 11 58 27Q57 43 40 45L25 59L28 45','none','stroke="#159df4" stroke-dasharray="5 4" stroke-width="4"'))
add('magic-wand',path('M10 53L34 27L42 34L17 60Z','url(#paper)')+path('M44 4L47 12L56 13L49 19L51 28L43 23L36 28L37 19L31 13L40 12Z','url(#gold)')+path('M56 31L58 36L63 38L58 40L56 45L54 40L49 38L54 36Z','url(#gold)'))
add('move',path('M26 14H20L32 3L44 14H38V26H50V20L61 32L50 44V38H38V50H44L32 61L20 50H26V38H14V44L3 32L14 20V26H26Z'))
add('crop',line('M14 4V49H59M4 15H48V59','#9aa8ba',7)+line('M14 16H47V48','#e8f3ff',2))
add('resize',rect(6,6,52,52,'url(#slate)',1)+path('M10 10H54V54H10Z','url(#blue)','stroke="none"')+line('M19 45L46 18M33 18H46V31M19 32V45H32','#c6eaff',4))
# Row 3: painting tools.
add('canvas-size',rect(5,23,54,35,'url(#paper)',2)+rect(10,28,44,25,'url(#blue)',0)+path('M12 50L26 34L35 44L42 37L53 50Z','#83d475','stroke="none"')+line('M7 10H56M7 10L14 5M7 10L14 15M56 10L49 5M56 10L49 15','#159ff5',3))
add('pencil',pencil)
add('brush',brush)
add('spray',rect(13,31,27,29,'url(#blue)',2)+path('M17 31V20H29V13H36V31Z','url(#paper)')+circle(47,21,6)+circle(53,8,4)+circle(60,29,3)+circle(42,9,3)+circle(51,38,3))
add('eraser',path('M6 40L35 9Q39 5 43 9L59 24Q63 28 59 32L33 59H23Z','url(#red)')+path('M6 40L21 25L44 48L33 59H23Z','url(#paper)')+line('M22 23L45 46','#ffd2df',2))
add('bucket',group(rect(13,14,30,31,'url(#paper)',2)+path('M14 34H42V44H14Z','url(#blue)')+path('M17 14V8Q28 -2 39 8V14','none'), 'rotate(-40 28 30)')+path('M49 36Q34 54 48 60Q62 56 49 36Z'))
add('eyedropper',path('M9 54L34 23L43 31L18 59L5 62Z')+path('M29 21L42 6Q49 0 57 8Q65 15 58 22L44 38Z','url(#slate)')+line('M14 51L33 29','#9feaff',2))
add('clone-stamp',path('M24 32V23Q10 6 31 4Q53 6 39 23V32Z','url(#gold)')+path('M9 37Q31 25 55 37V52H9Z','url(#gold)')+rect(7,51,50,8,'url(#slate)',1))
add('recolor',group(wheel(),'translate(2 2) scale(.9)')+group(brush,'translate(23 16) scale(.65)'))
add('text',path('M7 6H57V21H50L46 14H38V52L44 55V59H20V55L26 52V14H18L14 21H7Z','url(#slate)')+line('M12 10H52M30 16V50','#cad6e3',2))
add('line',line('M10 54L54 10','#148ef1',7)+circle(10,54,6)+circle(54,10,6))
add('curve',line('M8 55C48 57 18 18 55 9','#178df5',6)+circle(8,55,5)+circle(55,9,5))
# Row 4: shape gallery.
add('rectangle',rect(7,10,50,45)+rect(12,15,40,35,'none',0,'stroke="#8bd9ff" stroke-width="2"'))
add('rounded-rectangle',rect(6,9,52,47,r=13)+rect(12,15,40,35,'none',9,'stroke="#8bd9ff" stroke-width="2"'))
add('ellipse',circle(32,32,26)+circle(32,32,21,'none','stroke="#8bd9ff" stroke-width="2"'))
add('hexagon',path('M32 5L56 18V46L32 60L8 46V18Z')+path('M32 12L50 23V42L32 53L14 42V23Z','none','stroke="#9ee2ff" stroke-width="2"'))
add('triangle',path('M32 5L61 58H3Z')+path('M32 17L52 52H12Z','none','stroke="#a3e0ff" stroke-width="2"'))
def star(n,inner,outer=29):
    pts=[]
    for i in range(n*2):
        a=-math.pi/2+i*math.pi/n;r=outer if i%2==0 else inner
        pts.append(f'{32+math.cos(a)*r:.2f} {32+math.sin(a)*r:.2f}')
    return 'M'+'L'.join(pts)+'Z'
add('star',path(star(5,13)))
add('star-outline',path(star(5,13),'none','stroke="#163750" stroke-width="4"'))
add('speech-bubble',path('M58 28A26 24 0 1 0 18 49L13 61L32 52Q57 51 58 28Z'))
add('rounded-speech-bubble',path('M15 7H49Q59 7 59 18V41Q59 51 45 51L27 61V51H15Q5 51 5 40V18Q5 7 15 7Z'))
add('heart',path('M32 59L8 35Q-3 22 8 11Q20 1 32 16Q44 1 56 11Q67 22 56 35Z','url(#red)')+line('M10 22Q11 11 22 14','#ffc2d3',3))
add('lightning',path('M38 3L8 36H29L24 61L59 26H37Z','url(#gold)'))
add('burst',path(star(12,23)))
# Row 5: layers and transforms.
add('add-layer',group(layer,'translate(-1 -1) scale(.86)')+badge('+','url(#green)'))
add('delete-layer',group(layer,'translate(-1 -1) scale(.86)')+badge('-','url(#red)'))
add('duplicate-layer',rect(20,7,37,44)+rect(7,20,37,40,'url(#paper)',1))
add('merge-down',group(layer,'translate(0 15) scale(1 .75)')+path('M24 3H40V19H50L32 37L14 19H24Z','url(#green)'))
add('show-layer',path('M3 32Q30 -3 61 32Q32 67 3 32Z','url(#paper)')+circle(32,32,15)+circle(32,32,7,'#182535')+circle(36,27,4,'#fff','stroke="none"'))
add('hide-layer',path('M3 32Q30 -3 61 32Q32 67 3 32Z','url(#paper)')+circle(32,32,15,'url(#slate)')+line('M12 56L52 7','#f04b62',6))
add('move-layer-up',group(layer,'translate(0 15) scale(1 .75)')+path('M24 37H40V21H50L32 3L14 21H24Z','url(#green)'))
add('move-layer-down',group(layer,'translate(0 15) scale(1 .75)')+path('M24 3H40V19H50L32 37L14 19H24Z','url(#green)'))
rotate=path('M55 24L55 5L47 12Q17 -4 6 24Q-2 52 25 60Q42 65 54 50L45 43Q31 57 19 45Q6 32 19 20Q30 9 40 21L32 28Z')
add('rotate-clockwise',rotate)
add('rotate-counterclockwise',group(rotate,'translate(64 0) scale(-1 1)'))
add('flip-horizontal',path('M6 17L25 32L6 47ZM58 17L39 32L58 47Z')+line('M32 4V60','#6bbcff',3,'stroke-dasharray="5 4"'))
add('flip-vertical',path('M17 6L32 25L47 6ZM17 58L32 39L47 58Z')+line('M4 32H60','#6bbcff',3,'stroke-dasharray="5 4"'))
# Row 6: adjustments and effects.
rays=''.join(f'M{32+math.cos(i*math.pi/4)*25:.1f} {32+math.sin(i*math.pi/4)*25:.1f}L{32+math.cos(i*math.pi/4)*30:.1f} {32+math.sin(i*math.pi/4)*30:.1f}' for i in range(8))
add('brightness',line(rays,'#ffcf29',5)+circle(32,32,18,'url(#gold)')+circle(32,32,13,'none','stroke="#fff59a" stroke-width="2"'))
add('contrast',circle(32,32,26,'url(#paper)')+path('M32 6A26 26 0 0 1 32 58Z','url(#slate)'))
def rgb_circles(shift=0):
    return circle(32,19,17,'#ef334a','fill-opacity=".85"')+circle(19,42,17,'#60cb39','fill-opacity=".85"')+circle(45,42,17,'#214eff','fill-opacity=".85"')+circle(32,33,9,'#f5f4be','fill-opacity=".8" stroke="none"')
add('hue-saturation',rgb_circles())
add('curves',rect(6,6,52,52,'url(#slate)',1)+line('M23 9V55M41 9V55M9 23H55M9 41H55','#57687f',1)+line('M9 54C35 55 20 15 55 10','#49bfff',4)+circle(27,35,5)+circle(52,12,5))
add('blur',path('M32 4Q19 26 13 38Q4 60 32 61Q60 60 51 38Z')+line('M23 34Q12 49 25 53','#a1e8ff',3))
add('sharpen',path('M32 5L60 57H4Z')+path('M31 18L47 48L34 40L25 46L18 48Z','#abe9ff','stroke="none"'))
add('levels',rect(5,43,7,17)+rect(13,35,7,25)+rect(21,22,7,38)+rect(29,10,7,50)+rect(37,25,7,35)+rect(45,40,7,20)+rect(53,49,7,11))
add('color-balance',group(rgb_circles(),'rotate(15 32 32)'))

assert len(icons)==68
# Original PaintCraft action icon, separate from the supplied 68-icon sheet.
add('clear-canvas',
    path('M28 35L49 7Q52 3 56 6Q60 9 57 13L35 41Z', 'url(#blue)')
    + line('M34 31L52 8', '#a4e6ff', 2)
    + path('M19 34L32 29L42 38L37 49Z', 'url(#slate)')
    + path('M19 39L36 47L30 59Q13 61 5 50L15 39Z', 'url(#gold)')
    + line('M19 44L12 53M25 47L20 57M31 49L27 58', '#bd7a15', 2)
    + line('M43 54H55M48 47H59', '#71c8ff', 2))

# Original Classic gallery additions; these have no source-sheet coordinates.
add('polygon', path('M7 13H32L24 30H56L43 55H7Z'))
add('right-triangle', path('M8 7L57 56H8Z'))
add('diamond', path('M32 4L60 32L32 60L4 32Z'))
add('pentagon', path('M32 4L60 25L49 58H15L4 25Z'))
add('right-arrow', path('M5 21H34V7L60 32L34 57V43H5Z'))
add('left-arrow', path('M59 21H30V7L4 32L30 57V43H59Z'))
add('up-arrow', path('M21 59V30H7L32 4L57 30H43V59Z'))
add('down-arrow', path('M21 5V34H7L32 60L57 34H43V5Z'))
add('rectangular-callout', path('M5 8H59V46H31L17 60V46H5Z'))
add('cloud-callout', path('M13 42C0 39 1 25 12 22C8 9 26 3 34 13C45 3 61 14 54 26C68 37 50 54 39 45C31 54 17 52 13 42Z'))
add('four-point-star', path(star(4, 9)))
add('six-point-star', path(star(6, 15)))
add('remove-background', rect(5, 7, 54, 50, 'url(#paper)') + line('M7 22H57M7 39H57M22 9V55M40 9V55', '#8293a8', 2) + path('M24 40L43 16L58 29L39 53H28Z', 'url(#red)') + path('M24 40L35 49L39 53H28Z', 'url(#paper)'))
# Original Classic transform choices, distinct from the 90-degree arrows.
add('rotate-180',
    path('M15 6H42L51 58H24Z', 'url(#paper)')
    + path('M15 6L51 58H24Z', 'url(#blue)')
    + line('M15 6L51 58', '#61788d', 2))
add('rotate-custom',
    line('M10 30A22 22 0 1 1 43 50', '#128ff5', 5)
    + path('M42 39L40 57L57 50Z', 'url(#blue)')
    + line('M31 46V29L40 17', '#61788d', 3)
    + circle(31, 29, 4, 'url(#paper)'))
add('share',
    line('M7 33V57H51V46', '#61788d', 5)
    + path('M34 5L60 25L34 44V32Q19 30 10 47Q6 17 34 18Z', 'url(#blue)'))
# Color wheel with an add badge for the color editor, not the balance adjustment.
segments = []
for i in range(12):
    a, b = (i * math.tau / 12, (i + 1) * math.tau / 12)
    x0, y0 = 29 + 26 * math.cos(a), 35 + 26 * math.sin(a)
    x1, y1 = 29 + 26 * math.cos(b), 35 + 26 * math.sin(b)
    rgb = tuple(round(c * 255) for c in colorsys.hsv_to_rgb(i / 12, 1, 1))
    segments.append(path(f'M29 35L{x0:.3f} {y0:.3f}A26 26 0 0 1 {x1:.3f} {y1:.3f}Z', '#%02x%02x%02x' % rgb, 'stroke="none"'))
add('edit-colors', ''.join(segments)
    + '<defs><radialGradient id="wheel-center"><stop stop-color="#fff"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></radialGradient></defs>'
    + circle(29, 35, 26, 'url(#wheel-center)', 'stroke="none"')
    + circle(51, 13, 10, 'url(#paper)', 'stroke="#61788d" stroke-width="2"')
    + line('M46 13H56M51 8V18', '#128ff5', 2))
add('pointer', path('M13 4L13 52L25 41L35 61L46 55L36 35H56Z', 'url(#paper)')
    + line('M19 17L19 38', '#59ceff', 2))
OUT.mkdir(parents=True,exist_ok=True)
manifest=[]
for i,(name,body) in enumerate(icons):
    label=name.replace('-',' ').title()
    svg=f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">\n<title>{label}</title>\n{DEFS}\n<g stroke="#101d2b" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">\n{body}\n</g>\n</svg>\n'
    (OUT/f'{name}.svg').write_text(svg)
    manifest.append({'name':name,'label':label,'source_row':i//12+1 if i<68 else None,'source_column':i%12+1 if i<68 else None,'file':f'{name}.svg'})
(OUT/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
(OUT/'gallery.html').write_text('<!doctype html><html lang="en"><meta charset="utf-8"><title>PaintCraft vector icon gallery</title><style>body{font:14px system-ui;background:#eceff3;color:#172332;margin:32px}main{display:grid;grid-template-columns:repeat(auto-fit,minmax(130px,1fr));gap:12px}figure{margin:0;padding:12px;text-align:center;background:white;border-radius:8px}img{width:64px;height:64px}figcaption{margin-top:8px}</style><h1>PaintCraft vector icons</h1><p>68 individual SVG adaptations of the user-supplied generated sheet, plus original PaintCraft action and Classic gallery icons.</p><main>'+''.join(f'<figure><img src="{r["file"]}" alt="{r["label"]}"><figcaption>{r["label"]}</figcaption></figure>' for r in manifest)+'</main></html>\n')
print(f'Wrote {len(icons)} vector icons, manifest and gallery to {OUT}')
