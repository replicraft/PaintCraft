# Release builds

The [release workflow](../.github/workflows/release.yml) follows the native runner,
portable archive and complete-release checks used by
[EarthCraft](https://github.com/therichronicles/EarthCraft/blob/main/.github/workflows/release.yml).
PaintCraft retains GPU rendering in every desktop build. It uses the locked Cargo
dependencies and Rust 1.99.0 from `rust-toolchain.toml`.

Normal helper builds enable `avif-sdr-codec` by default. Native release builds also
explicitly request `paintcraft-codec-worker/avif-sdr-codec`; a V4 compatibility
helper requires `--no-default-features`; V5 identity-only compatibility uses
`--no-default-features --features avif-codec`.
The pinned dav1d source is built statically with Meson 1.12.1 and Ninja 1.13.2;
no installed decoder library is used. Windows builds select the native MSVC
compiler; release Rust flags request `crt-static` and the C build matches with
Meson `b_vscrt=mt`. Normal/source builds without that target feature use `md`,
so the Rust and C runtime choices agree. Each runner executes actual helper and installed
CLI/MCP tests before packaging. These workflow changes still require execution
on all six targets; local Apple-silicon checks do not qualify the other targets.

Packaging reads the helper's embedded build record without executing it, then
writes an exact manifest for V4, identity-only V5 or complete V6. It refuses mixed,
stale or malformed records. Desktop and CLI packages include the helper,
manifest, `JPEGXR_LICENSE.txt` and `DAV1D_LICENSE.txt`. Portable archives keep
these beside the executable. Linux system packages place notices under
`/usr/share/licenses/paintcraft` and the manifest under `/usr/share/paintcraft`.

## Downloads

Names start with `paintcraft-<version>-`. For version 0.1.0:

| Platform | Architectures | Suffixes |
| --- | --- | --- |
| Windows | `windows-x64`, `windows-arm64` | `-portable.zip`, `.exe`, `.msi` |
| macOS | `macos-arm64`, `macos-x64` | `.dmg`, `-update.zip` |
| Linux | `linux-x64`, `linux-arm64` | `.deb`, `.rpm`, `.AppImage`, `.tar.gz` |

The assembled release has 24 downloads plus `SHA256SUMS.txt`. Each portable ZIP
or tar archive includes the application, its license and bundled font notices.
Fonts and interface assets are embedded in the executable. There are no source trees or build tools
inside the downloads. The checksums describe the final downloadable bytes.

## Build and publish

1. Set `[workspace.package].version` in the root `Cargo.toml`, update the lockfile
   and write `docs/releases/v<version>.md` with user-facing installation notes,
   changes and known limitations. Tag validation requires this file to be nonempty.
2. Run `python3 -m unittest discover -s scripts/tests -p 'test_package_*.py'`.
3. Use **Actions → Release → Run workflow** to build and assemble every platform
   without publishing. Download `paintcraft-release-<version>` from that run.
4. Check installation, document opening, rendering, file dialogs, upgrades and
   uninstall on the actual target systems. Automated build and installer checks
   do not establish interactive compatibility.
5. Push a matching `v<version>` tag when ready to publish. A tag that disagrees
   with Cargo fails before building. The tag run builds all platforms again,
   checks every asset and SHA-256 digest, creates/uploads a draft and then
   publishes a short authored summary with a link to the full changelog.
   An existing published release is refused. Prerelease versions
   produce prerelease downloads; MSI and Apple bundle metadata use the numeric
   major/minor/patch version. Use a new numeric version when upgrading an MSI.

Pull requests changing packaging also build the release matrix. Native build
artifacts and the assembled set are retained for seven days. No release is
published from a pull request or a build-only manual workflow run. No credentials are needed
for local packaging; tag publication uses the job's repository token.

To reuse completed builds, run the Release workflow with `artifacts-run` set to
the source run ID and `publish-tag` set to its existing version tag. Every required
platform build must have succeeded at that exact tagged commit. The workflow
skips compilation, downloads those artifacts, checks the complete current package
set and publishes it with a summary and a full changelog link. This supports removing an obsolete
target without repeating successful builds; it cannot reuse binaries for a
different source commit.

Like EarthCraft, release validation fetches the complete Git history and uses
git-cliff 2.14.2 with `cliff.toml` to generate a grouped `CHANGELOG.md`.
Download it from the separate `release-changelog-<version>` workflow artifact.
The authored installation notes remain in `docs/releases/v<version>.md`.
`cliff.toml` excludes documentation commits and classifies descriptive commits as well as Conventional Commits, with
explicit exceptions for historical subjects whose intent is ambiguous. Keep the
root `CHANGELOG.md` updated with `git-cliff --output CHANGELOG.md` when preparing
release notes. Check that every non-documentation release commit appears once and none fall under
"Other changes" before publishing. Note-only corrections after a tag can update
the GitHub release body without rebuilding the application downloads.

The `assemble` job refuses a missing architecture, extra/stale asset, empty file,
symlink or mismatched per-asset checksum. Only after all 24 checks succeed does
it write the sorted, LF-delimited `SHA256SUMS.txt` and remove intermediate checksum
sidecars. It runs `sha256sum --check` independently before uploading the final set.

## Platform details

Windows builds run natively on Windows x64 and ARM64. The VC runtime is linked
statically, so portable downloads do not require a separate VC redistributable.
The executable contains the PaintCraft icon and version resources. WiX 5.0.2
builds both MSI architectures from one template, with a stable upgrade identity,
Program Files installation and a Start menu shortcut. The MSI registers the
`.pcnd` extension, `PaintCraft.Document` ProgID, icon, Open With and Default Apps
capabilities. Its open command quotes both executable and document paths.
It does not write the user's protected `UserChoice` preference. Existing explicit
default-app choices remain controlled by Windows. CI installs, repairs and
uninstalls each MSI and checks the owned registry entries and installed binary.
Windows packages are not Authenticode-signed.

macOS requires 13.0 or newer. Apple Silicon and Intel use separate thin Rust
binaries and matching Swift launchers on native runners. Both retain the existing
Finder document-event launcher and `.pcnd` Launch Services declarations. DMGs
use `dmgbuild==1.6.7` with EarthCraft's 640 × 270 Finder window, 128-pixel icons
and arrow background, placing `PaintCraft.app` beside the Applications shortcut.
The `-update.zip` contains
that exact app, including its helper app, icon, metadata, signatures and executable
permissions. It uses forward-slash archive paths on every host. Both bundles are
ad-hoc signed and checked with `codesign --verify --deep --strict`; DMGs are checked
with `hdiutil verify`. These are not Developer ID signed or notarized. The release
verifier also mounts each DMG read-only, compares every app file against the
extracted ZIP and verifies both executable architectures and ZIP permissions.
No application is launched during these checks. The updater archive is ready
for a future updater consumer; this packaging work does not add
an in-app update service.

Linux packages build on Ubuntu 22.04 for glibc 2.35 or newer, with X11/Wayland
and Vulkan/OpenGL driver support. DEB and RPM include desktop, icon, AppStream,
license and `.pcnd` MIME metadata. Package-manager MIME/desktop triggers refresh
the relevant caches; no global default image handler is changed. Native dialogs
use the host's XDG desktop portal and require a portal backend suitable for the
desktop. AppImages include the X11, XKB and Wayland libraries loaded at runtime
in addition to discovered ELF dependencies, but still need a suitable graphics
driver. They can use FUSE or `APPIMAGE_EXTRACT_AND_RUN=1`; make the downloaded
AppImage executable before running it. Portable tar archives require host runtime
libraries. AppImages and portable archives do not install host MIME associations.
Package generation uses pinned cargo-packager 0.11.8 and cargo-generate-rpm 0.21.0.

## Local packaging

Build the native release executable with the appropriate Rust target first:

```sh
cargo build --locked --release -p paintcraft --target aarch64-apple-darwin
python3 scripts/packaging/package_release.py package --version 0.1.0 \
  --target aarch64-apple-darwin \
  --binary target/aarch64-apple-darwin/release/paintcraft --output dist
```

Use `x86_64-apple-darwin` for Intel. The packaging script checks the executable
header rather than assuming its filename describes its architecture. DMG
creation requires macOS, Xcode command-line tools and disk-image access. It does
not launch, install or register the app.

For Linux, run `scripts/packaging/package_release.py package` for the tar archive, then
`scripts/packaging/package_linux.py` with the same `--version`, `--target`, `--binary` and
`--output`. Install the pinned tools from the release-tools action first.
Windows uses the archive command followed by `scripts/packaging/package_windows.py` after
installing WiX 5.0.2. Windows release builds must use
`RUSTFLAGS=-C target-feature=+crt-static`, as in CI.

Once every platform's files and `.sha256` sidecars are together:

```sh
python3 scripts/packaging/package_release.py finalize --version 0.1.0 --output dist
python3 scripts/packaging/package_release.py verify --version 0.1.0 --output dist
cd dist
sha256sum --check SHA256SUMS.txt
```

### Headless CLI distribution

Each of the six native targets also produces a separate `paintcraft-cli-<version>-<platform>.zip`
(Windows) or `.tar.gz` (macOS, Linux). These archives contain the CLI, its helper,
manifest and licenses,
and their hashes are included in `SHA256SUMS.txt`. They are separate from desktop installers.
Extract all archive files together into a directory on your PATH, keeping the
helper, manifest and notices beside the CLI. The Unix archives preserve executable permissions.

From a checkout, run `sh scripts/packaging/install_cli.sh` on Unix or
`./scripts/packaging/install_cli.ps1` on Windows. Python 3.12 or newer is required.
The default downloads the latest stable complete CLI archive and `SHA256SUMS.txt`
from the official GitHub release, checks the archive digest and verifies the helper's
architecture, embedded build contract, manifest and license files before installation.
These checks provide integrity against the published checksums, not an independent
release signature. Incomplete older archives are refused; use `--source` until a
release containing the helper is published.

All files are installed together under an owned private prefix's `bin` directory.
The defaults are `~/.local/share/paintcraft-cli/bin` on Unix and
`%LOCALAPPDATA%/PaintCraft/cli/bin` on Windows. Add that complete directory to PATH;
the installer prints its path and never edits shell profiles or system settings.
Use `--prefix /your/private/prefix` to choose a real directory owned by you, or
`--version 0.1.0` to select a published version. Existing foreign files, symlinked
paths and incompatible installations are refused. Updates stage a complete verified
payload before replacing the old directory; failed publication restores the old
installation, or retains its directory for manual recovery if restoration also fails.
If removing the old files fails after publication, installation still succeeds
and prints the retained directory's location for cleanup.
Do not run another installer while `.install-lock` is present; inspect any leftover
lock and recovery directory after an interrupted process.

`--source` builds this checkout's CLI **and AVIF-enabled helper** using the locked
Rust dependencies, a native C compiler, Meson 1.12.1 and Ninja 1.13.2. It generates
the exact manifest, bundles both decoder notices and installs through the same
archive validation and transaction. It does not invoke `cargo install` or install
Rust or build tools for you. Source builds use the checkout's version and cannot
be combined with `--version` or `--archive`.

For offline installation, supply `--archive PATH --checksums PATH --version VERSION`
for an official complete archive and its `SHA256SUMS.txt`. The checksum and full
payload checks are identical to the default download route. Tests use only temporary
private prefixes; synthetic archive checks for six targets do not qualify native
execution on the other systems.
