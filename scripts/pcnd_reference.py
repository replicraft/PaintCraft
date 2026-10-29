#!/usr/bin/env python3
"""Independent raw PCND 1 writer and live persistence-fixture generator.

Batch semantics come from the reference pack; this module owns only the adopted
binary persistence bytes. It never calls the Rust codec or reads ZIP projects.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import uuid

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
MAGIC = b'PCND\x01\r\n\x1a'


def integer(value):
    assert 0 <= value < 1 << 64
    result = bytearray()
    while value >= 128:
        result.append((value & 127) | 128)
        value >>= 7
    result.append(value)
    return result


def string(value):
    encoded = value.encode('utf-8')
    return integer(len(encoded)) + encoded


def canonical(document):
    attachment = document.get('source_metadata')
    result = bytearray(MAGIC if attachment is None else b'PCND\x02\r\n\x1a')
    result += uuid.UUID(document['document_id']).bytes
    result += uuid.UUID(document['state_id']).bytes
    for dimension in document['canvas']:
        result += integer(dimension)
    title = document['title']
    result.append(int(title is not None))
    if title is not None:
        result += string(title)
    if attachment is not None:
        result += attachment
    result += integer(len(document['layers']))
    for layer in document['layers']:
        result += uuid.UUID(layer['id']).bytes + string(layer['name'])
        result += bytes([layer['visible'], layer['opacity']])
        for coordinate in layer['origin']:
            result += integer((coordinate << 1) ^ (coordinate >> 63))
        width, height = layer['width'], layer['height']
        result += integer(width) + integer(height) + bytes(layer['default'])
        pixels = layer['pixels']
        tiles = sorted({(y // 256, x // 256) for (x, y), color in pixels.items()
                        if tuple(color) != tuple(layer['default'])})
        for by, bx in tiles:
            w, h = min(256, width - bx * 256), min(256, height - by * 256)
            raw = b''.join(bytes(pixels.get((bx * 256 + x, by * 256 + y), layer['default']))
                           for y in range(h) for x in range(w))
            result.append(1)
            result += integer(bx) + integer(by) + integer(len(raw)) + raw
        result.append(0)
    return bytes(result) + hashlib.sha256(result).digest()


def specimen():
    return canonical(dict(document_id='33333333-3333-4333-8333-333333333333',
                          state_id='11111111-1111-4111-8111-111111111111', canvas=(2, 2),
                          title='Generated PCND contract specimen', layers=[dict(
                              id='22222222-2222-4222-8222-222222222222', name='Specimen',
                              visible=True, opacity=255, origin=(0, 0), width=2, height=2,
                              default=(0, 0, 0, 0), pixels={
                                  (0, 0): (255, 0, 0, 0), (1, 0): (0, 255, 0, 255),
                                  (0, 1): (0, 0, 255, 128), (1, 1): (255, 255, 255, 255)})]))


def projection(doc):
    assert set(doc.metadata) <= {'title'}
    return dict(document_id=doc.document_id, state_id=doc.state_id, canvas=doc.canvas,
                title=doc.metadata.get('title'), layers=[dict(
                    id=layer.id, name=layer.name, visible=layer.visible,
                    opacity=layer.opacity, origin=layer.origin, width=layer.width,
                    height=layer.height, default=layer.default, pixels=layer.pixels)
                    for layer in (doc.layers[key] for key in doc.roots)])


def refresh(reference_pack, write):
    sys.path.insert(0, str(reference_pack / 'scripts'))
    from reference_engine import OracleSession, new_document, render, selection_json
    from conformance_vectors import snapshot
    checks = 0
    for path in sorted((ROOT / 'conformance').glob('*.json')):
        original = path.read_text()
        text = original
        vector = json.loads(original)
        new = vector['initial']['new']
        session = OracleSession(new_document(new['width'], new['height'], new['background'], new['seed'], new['policy']))
        for step in vector['steps']:
            action, want = step['action'], step.get('expect')
            if action == 'apply':
                actual = session.apply(step['batch'])
                actual.pop('message', None)
                assert actual == want, path
            elif action in ('undo', 'redo'):
                if want == {'available': False}:
                    try:
                        getattr(session, action)()
                    except ValueError:
                        pass
                    else:
                        raise AssertionError('unexpected history navigation')
                else:
                    assert getattr(session, action)() == want, path
            elif action == 'mark-saved':
                session.mark_saved()
            elif action == 'check':
                assert snapshot(session.doc) == want['document'], path
                assert selection_json(session.selection) == want['selection'], path
                assert session.dirty == want['dirty'], path
                assert hashlib.sha256(render(session.doc)).hexdigest() == want['render_sha256'], path
                digest = hashlib.sha256(canonical(projection(session.doc))).hexdigest()
                text = text.replace(want['pcnd_sha256'], digest)
                checks += 1
            else:
                raise AssertionError(action)
        if write:
            path.write_text(text)
        else:
            assert text == original, f'{path}: persistence digest drift'
    path = ROOT / 'conformance/minimal.pcnd'
    if write:
        path.write_bytes(specimen())
    else:
        assert path.read_bytes() == specimen(), 'binary specimen drift'
    print(f'{checks} binary persistence checks; batch results, snapshots and renders unchanged.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference-pack', type=Path, required=True)
    parser.add_argument('--write', action='store_true')
    arguments = parser.parse_args()
    refresh(arguments.reference_pack.resolve(), arguments.write)
