# Opening PaintCraft documents from the desktop

On macOS, Finder needs an application bundle to discover PaintCraft as an editor
for `.pcnd` documents. Running `cargo run` alone does not install that association.

Build a local bundle from the repository root with Python 3.11 or later:

```sh
python3 scripts/package_macos.py
```

The release executable, native Finder launcher and existing PaintCraft icon are placed in
`target/macos/PaintCraft.app`. The command signs the local bundle with an ad hoc
signature. It does not launch the app, install it or register a default handler.
This is a local developer bundle, without Developer ID signing or notarization.
Packaging requires Xcode command-line tools to compile the small Swift launcher;
the installed application does not require a compiler. macOS 13 or later is required.

To install a copy in your own Applications folder and register it with macOS:

```sh
python3 scripts/package_macos.py --install --register
```

Add `--replace` when updating an existing PaintCraft bundle. Replacement refuses
symlinks and bundles with another application's identifier. Failed preparation or
signing leaves the previous bundle intact. Registration calls Launch Services;
it does not write a default-handler preference. To choose PaintCraft explicitly,
select a `.pcnd` file in Finder, use **Get Info > Open with > PaintCraft**, and
choose **Change All** if that should apply to every `.pcnd` document.

Use `--debug` for a debug build, or `--binary /path/to/paintcraft` to package an
already-built native macOS executable. Cargo's reported executable path is used,
including when the Cargo target directory is customized. `--output` changes the
bundle destination. Packaging and installation never open an application window.

The bundle exports the owned type `io.github.replicraft.PaintCraft.pcnd`, conforming
to `public.data` and `public.content`, with extension `pcnd` and MIME type
`application/x-paintcraft`. Its document role is `Editor` and handler rank is
`Owner`; the existing `paintcraft-icon.icns` supplies the document and application
icons. These declarations follow Apple's
[custom document type guidance](https://developer.apple.com/documentation/uniformtypeidentifiers/defining-file-and-data-types-for-your-app)
and [handler rank definition](https://developer.apple.com/documentation/bundleresources/information-property-list/cfbundledocumenttypes/lshandlerrank).

The bundle's native accessory launcher receives Finder file-open events during
startup and while editor windows are open. It starts a separate Rust editor process
and window for each requested file, passing the path as one process argument.
Existing windows and their unsaved work are not replaced. A normal application
launch opens a blank window after a short initial event-coalescing interval; an
initial file-open request cancels that blank launch. The launcher exits after its
last child editor process ends. It has no separate Dock icon. The Rust editor lives in a nested application bundle
with a distinct identifier, so it cannot take over the launcher's file-open events.
This provides file
opening in separate windows, not tab reuse or single-process window coordination.

The packaging tests validate metadata, resource layout, safe replacement and opt-in
registration without opening windows or changing Launch Services state:

```sh
python3 -B -m unittest discover -s scripts/tests -p 'test_package_macos.py'
```

The Swift launcher also has `--validate-only` mode for checking the packaged editor
path and exact argument preservation without creating an application or window.
An explicit native event test builds an isolated temporary bundle with a unique
identifier and no document-type registrations, then asks Launch Services to open
files before and after launch. Dummy native editor processes log arguments and
exit without creating windows. This verifies cold/running event delivery, exact
paths and suppression of a duplicate blank launch. It passed on the local macOS
host. It does not validate the real editor UI, default-handler selection or other
macOS profiles.

```sh
PAINTCRAFT_TEST_NATIVE_OPEN=1 python3 -B -m unittest discover -s scripts/tests -p 'test_package_macos.py'
```

Linux MIME/desktop files and a Windows installer or registration workflow are not
provided by the current packaging setup. Those platforms need their own opt-in
integration and platform validation before claiming file associations there.
