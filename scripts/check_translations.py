#!/usr/bin/env python3
"""Check interface catalog parity, duplicate IDs, and literal t! message coverage."""
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
LOCALES = ROOT / 'crates/presentation/paintcraft-ui/locales'
LANGUAGES = ('en', 'de', 'es', 'hi', 'ja', 'pt-BR', 'zh-CN')
DOMAINS = ('app', 'common', 'classic', 'studio', 'settings')


def catalog(locale, failures):
    result = {}
    folder = LOCALES / locale
    actual = {path.stem for path in folder.glob('*.ftl')}
    if actual != set(DOMAINS):
        failures.append(f'{locale}: expected domains {DOMAINS}, found {sorted(actual)}')
    for domain in DOMAINS:
        path = folder / f'{domain}.ftl'
        if not path.exists():
            continue
        current = None
        for number, line in enumerate(path.read_text().splitlines(), 1):
            if not line.strip() or line.startswith('#'):
                continue
            match = re.match(r'^([a-z][a-z0-9-]*)\s*=\s*(.*)$', line)
            if match:
                current, value = match.groups()
                if current in result:
                    failures.append(f'{path}:{number}: duplicate message {current}')
                result[current] = (domain, value)
            elif line[0].isspace() and current:
                owner, value = result[current]
                result[current] = (owner, value + '\n' + line)
            else:
                failures.append(f'{path}:{number}: invalid message line')
    return result


def key(source):
    result = re.sub('[^a-z0-9]+', '-', source.lower()).strip('-')
    return result + ('-more' if source.endswith('…') else '')


def main():
    failures = []
    english = catalog('en', failures)
    arguments = lambda value: set(re.findall(r'\$([A-Za-z][A-Za-z0-9_-]*)', value))
    for locale in LANGUAGES[1:]:
        translated = catalog(locale, failures)
        for message in english.keys() | translated.keys():
            if message not in english or message not in translated:
                failures.append(f'{locale}/{message}: missing or unexpected message')
                continue
            domain, source = english[message]
            other_domain, target = translated[message]
            if domain != other_domain:
                failures.append(f'{locale}/{message}: wrong interface file')
            if arguments(source) != arguments(target):
                failures.append(f'{locale}/{message}: mismatched Fluent arguments')
            if not target.strip():
                failures.append(f'{locale}/{message}: empty translation')
    call = re.compile(r'\b(t!|tr|translate|tr_format|format_message)\(\s*[^,]+,\s*"([^"\n]*)"')
    roots = ['crates/presentation', 'apps/paintcraft/src', 'apps/paintcraft-web/src']
    for directory in roots:
        for path in (ROOT / directory).rglob('*.rs'):
            if 'tests' in str(path) or path.name == 'i18n.rs':
                continue
            source = path.read_text().split('#[cfg(test)]')[0]
            for match in call.finditer(source):
                text = match[2]
                if not text or text == 'PaintCraft Studio':
                    continue
                formatted = match[1] in ('tr_format', 'format_message') or '{ $' in text
                message = ('format-' if formatted else '') + key(text)
                if message not in english:
                    line = source.count('\n', 0, match.start()) + 1
                    failures.append(f'{path.relative_to(ROOT)}:{line}: missing {message}: {text}')
    if failures:
        print('\n'.join(failures), file=sys.stderr)
        return 1
    print(f'{len(english)} messages in {len(DOMAINS)} interfaces cover all {len(LANGUAGES)} locales and UI literals.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
