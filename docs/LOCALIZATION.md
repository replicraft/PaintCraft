# Interface languages

Desktop and web offer English, German, Spanish, Hindi, Japanese, Brazilian
Portuguese and Simplified Chinese in Settings → Language. Language names appear
in their own language. Desktop previews the choice in its Apply/Cancel dialog;
Cancel restores the previous choice. Browser settings take effect immediately.
Both hosts persist the language independently of image documents. Existing
`en-US` preferences remain compatible, and unknown IDs are rejected.

## Catalog layout

Fluent resources live in `crates/presentation/paintcraft-ui/locales`:

```text
locales/
  en/
    app.ftl
    common.ftl
    classic.ftl
    studio.ftl
    settings.ftl
  de/
  es/
  hi/
  ja/
  pt-BR/
  zh-CN/
```

Every language has the same five files and message IDs. `app.ftl` contains host
workflow messages, file operations and recovery. `common.ftl` contains shared
actions, tools, colors and dialogs. `classic.ftl` and `studio.ftl` contain their
interface-specific chrome. `settings.ftl` contains preferences. IDs are unique
across the files because each locale is loaded into one Fluent bundle. Shared
labels belong in `common.ftl` rather than being duplicated across interfaces.

## Translating UI code

Import `paintcraft_ui::t` once at the top of the module:

```rust,ignore
use paintcraft_ui::t;

let label = t!(ui.ctx(), "Settings");
let status = t!(ui.ctx(), "Opened { $detail }", detail = filename);
```

The first argument selects the egui context's language. Literal English labels
map to stable Fluent message IDs; formatted templates use the `format-` prefix.
Named arguments are values, never translation keys. An existing slice of named
string values is also accepted by the macro. Keep translations outside
context-locked accessibility callbacks.

User-entered text, filenames, font family names and command identifiers are not
translated. English source text remains the fallback for unknown messages.
Low-level diagnostic payloads and CLI machine-readable receipts retain their
existing contracts. Adding languages does not imply that every external error
message is localized or that a native speaker has reviewed every workflow.

## Fonts

Selawik remains the primary interface font. Bundled Noto Sans JP, Noto Sans SC
and Noto Sans Devanagari provide Japanese, Simplified Chinese and Hindi coverage
without relying on installed system fonts. Chinese selects SC before JP in the
fallback list so shared Han characters use the appropriate regional forms.
All language names remain readable before selecting a language. The same font
collection is available to document text on desktop and web. Each added font
includes its SIL Open Font License and a source/hash record.

The current egui text renderer shapes complex scripts, including Devanagari
conjuncts. Document text uses the existing cosmic-text renderer. Fonts are
reinstalled only when the context's language changes, rather than every frame.

## Validation

Run `python3 scripts/check_translations.py` to check all seven locales for
matching interface files, message IDs and Fluent arguments, duplicate IDs and
missing UI literals. Rust tests parse every resource and format every message
with sample arguments. Additional tests check context isolation, macro argument
preservation, bundled glyph coverage and Devanagari shaping. Desktop and web
GUI tests select the actual language options, check translated settings, and
verify document semantics remain unchanged; desktop also checks persistence.

Fluent bundles are cached per thread, while language belongs to each egui
context. Native menus update labels without changing action IDs. OS-supplied
standard menu entries retain the host system's localization. Native file-picker
titles and filter labels are localized at the presentation boundary.
