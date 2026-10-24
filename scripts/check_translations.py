#!/usr/bin/env python3
"""Catch missing Fluent entries and mismatched arguments in UI source templates."""
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
LOCALES = ROOT / 'crates/presentation/paintcraft-ui/locales'


def catalog(locale):
    return dict(line.split(' = ', 1) for line in (LOCALES / f'{locale}.ftl').read_text().splitlines())


def key(source):
    result = re.sub('[^a-z0-9]+', '-', source.lower()).strip('-')
    return result + ('-more' if source.endswith('…') else '')


def main():
    english, japanese = catalog('en-US'), catalog('ja')
    failures = []
    for message in english.keys() | japanese.keys():
        if message not in english or message not in japanese:
            failures.append(f'{message}: missing locale')
            continue
        args = lambda value: set(re.findall(r'\{ \$(\w+) \}', value))
        if args(english[message]) != args(japanese[message]):
            failures.append(f'{message}: mismatched Fluent arguments')
    # Match literal calls with context expressions such as ui.ctx(), &ctx, &self.context.
    call = re.compile(r'\b(tr|translate|tr_format|format_message)\(\s*[^,]+,\s*"([^"\n]*)"')
    roots = ['crates/presentation', 'apps/paintcraft/src', 'apps/paintcraft-web/src']
    for directory in roots:
        for path in (ROOT / directory).rglob('*.rs'):
            if 'tests' in str(path) or path.name == 'i18n.rs':
                continue
            source = path.read_text()
            # Unit test examples and their deliberate unknown messages are excluded.
            source = source.split('#[cfg(test)]')[0]
            for match in call.finditer(source):
                text = match[2]
                if not text or text == 'PaintCraft Studio':
                    continue
                message = ('format-' if match[1] in ('tr_format', 'format_message') else '') + key(text)
                if message not in english:
                    line = source.count('\n', 0, match.start()) + 1
                    failures.append(f'{path.relative_to(ROOT)}:{line}: missing {message}: {text}')
    if failures:
        print('\n'.join(failures), file=sys.stderr)
        return 1
    print('UI translation literals and Fluent arguments are covered in both locales.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
