# Interface languages

Settings includes a Language dropdown with English and Japanese on desktop and
web. The desktop previews the choice in its existing Apply/Cancel dialog; Cancel
restores the previous language. Browser settings take effect immediately. Both
hosts persist the choice without storing it in image documents. Existing settings
without a language keep English; unknown language IDs are rejected.

Shared menus, action buttons, tools, brushes, settings and form captions use
Fluent resources in `paintcraft-ui/locales`. English source labels remain the
fallback for messages that have not been translated. Low-level diagnostic errors,
some generated status sentences and remaining prototype help text still use
English. User-entered text, filenames, font family names and command identifiers
are never translated. CLI diagnostics and machine-readable receipts retain their
existing English/stable identifier contracts.

The application keeps Selawik as its primary interface font. Noto Sans JP supplies
Japanese glyphs in both UI and canvas text, including when no Japanese system
fonts are installed. It is also available in the browser's Default font chooser.
The font is bundled with its SIL Open Font License and source/hash record.

Fluent bundles are cached per thread, while the selected language belongs to each
egui context. Two editor contexts can use different languages. The native menu
updates its action labels and File/Edit/View/Window headings without rebuilding
the menu or changing action IDs. OS-supplied standard menu items retain the host
system's localization. Application-supplied native file-picker titles/filter
labels and the browser HTML download prompt receive translated strings from the
frontend; the platform adapter does not import the UI crate.

Tests parse every bundled message, check Japanese catalog coverage, fallback and
context isolation, drive the actual browser dropdown, verify preference round
trips and switch/Cancel/Apply through every desktop skin and Studio while checking
that document pixels are unchanged. These checks do not substitute for a native
Japanese language review of every workflow.

The implementation uses the [Fluent Rust runtime](https://docs.rs/fluent-bundle/0.16.0/fluent_bundle/).
