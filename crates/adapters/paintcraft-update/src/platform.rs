//! What this process is and how it was installed, which decides the asset and the update plan.
//!
//! Detection is a pure function of [`DetectInput`] so every rule is unit-tested on any host.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use semver::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Macos,
    Windows,
    Linux,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X64,
    Arm64,
    Other,
}

/// How this copy of PaintCraft is installed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InstallKind {
    /// `X.app/Contents/MacOS/<exe>`; `bundle` is `X.app`.
    MacosBundle { bundle: PathBuf },
    /// Installed by `paintcraft-<v>-windows-<arch>.msi` into `%ProgramFiles%\PaintCraft` (per machine, see
    /// `packaging/windows.wxs`: `Scope="perMachine"`, `INSTALLFOLDER` under `ProgramFiles6432Folder`).
    WindowsMsi { dir: PathBuf },
    /// Any other Windows location: the files of `-portable.zip`, `dir` holds the executables.
    WindowsPortable { dir: PathBuf },
    /// Started from an AppImage; `path` is `$APPIMAGE`.
    LinuxAppImage { path: PathBuf },
    /// deb, rpm, tarball, a `cargo` build, a debug build or anything unrecognised: never replaced
    /// in place.
    Manual(ManualReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualReason {
    DevBuild,
    Packaged,
    UnsupportedPlatform,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
    pub install: InstallKind,
}

/// Everything detection looks at, injectable for tests.
#[derive(Debug, Clone)]
pub struct DetectInput {
    pub os: Os,
    pub arch: Arch,
    pub exe: Option<PathBuf>,
    /// `$APPIMAGE`.
    pub appimage: Option<OsString>,
    /// `$APPDIR`, bound to the canonical running executable before in-place updates.
    pub appdir: Option<OsString>,
    /// `%ProgramFiles%` and `%ProgramW6432%` (Windows only; both may be the same).
    pub program_files: Vec<PathBuf>,
    pub debug_build: bool,
}

impl DetectInput {
    pub fn from_process() -> Self {
        Self {
            os: if cfg!(target_os = "macos") {
                Os::Macos
            } else if cfg!(windows) {
                Os::Windows
            } else if cfg!(target_os = "linux") {
                Os::Linux
            } else {
                Os::Other
            },
            arch: if cfg!(target_arch = "x86_64") {
                Arch::X64
            } else if cfg!(target_arch = "aarch64") {
                Arch::Arm64
            } else {
                Arch::Other
            },
            exe: std::env::current_exe()
                .ok()
                .map(|path| std::fs::canonicalize(&path).unwrap_or(path)),
            appimage: std::env::var_os("APPIMAGE"),
            appdir: std::env::var_os("APPDIR"),
            program_files: ["ProgramFiles", "ProgramW6432"]
                .iter()
                .filter_map(|name| std::env::var_os(name).map(PathBuf::from))
                .collect(),
            debug_build: cfg!(debug_assertions),
        }
    }
}

impl Platform {
    pub fn detect() -> Self {
        let mut platform = Self::from_input(&DetectInput::from_process());
        if cfg!(target_os = "linux") && Path::new("/.flatpak-info").is_file() {
            platform.install = InstallKind::Manual(ManualReason::Packaged);
        }
        platform
    }

    pub fn from_input(input: &DetectInput) -> Self {
        Self {
            os: input.os,
            arch: input.arch,
            install: detect_install(input),
        }
    }

    /// `windows-x64`, `macos-arm64`, ...: the platform part of every asset name.
    pub fn id(&self) -> Option<&'static str> {
        Some(match (self.os, self.arch) {
            (Os::Windows, Arch::X64) => "windows-x64",
            (Os::Windows, Arch::Arm64) => "windows-arm64",
            (Os::Macos, Arch::Arm64) => "macos-arm64",
            (Os::Macos, Arch::X64) => "macos-x64",
            (Os::Linux, Arch::X64) => "linux-x64",
            (Os::Linux, Arch::Arm64) => "linux-arm64",
            _ => return None,
        })
    }

    /// The exact asset file name for `version`, or `None` for a manual install.
    pub fn asset_name(&self, version: &Version) -> Option<String> {
        let id = self.id()?;
        let suffix = match &self.install {
            InstallKind::MacosBundle { .. } => "-update.zip",
            InstallKind::WindowsMsi { .. } => ".msi",
            InstallKind::WindowsPortable { .. } => "-portable.zip",
            InstallKind::LinuxAppImage { .. } => ".AppImage",
            InstallKind::Manual(_) => return None,
        };
        Some(format!("paintcraft-{version}-{id}{suffix}"))
    }
}

fn detect_install(input: &DetectInput) -> InstallKind {
    if input.debug_build {
        return InstallKind::Manual(ManualReason::DevBuild);
    }
    let Some(exe) = input.exe.as_deref() else {
        return InstallKind::Manual(ManualReason::Packaged);
    };
    if is_cargo_output(exe) {
        return InstallKind::Manual(ManualReason::DevBuild);
    }
    let kind = match input.os {
        Os::Macos => macos_bundle(exe).map(|bundle| InstallKind::MacosBundle { bundle }),
        Os::Windows => windows_kind(exe, &input.program_files),
        Os::Linux => appimage(input, exe).map(|path| InstallKind::LinuxAppImage { path }),
        Os::Other => return InstallKind::Manual(ManualReason::UnsupportedPlatform),
    };
    let kind = kind.unwrap_or(InstallKind::Manual(ManualReason::Packaged));
    if Platform::id(&Platform {
        os: input.os,
        arch: input.arch,
        install: kind.clone(),
    })
    .is_none()
    {
        return InstallKind::Manual(ManualReason::UnsupportedPlatform);
    }
    kind
}

/// A binary under a Cargo `target` directory (marked by `CACHEDIR.TAG`) is a developer build.
fn is_cargo_output(exe: &Path) -> bool {
    exe.ancestors().skip(1).any(|dir| {
        dir.file_name().is_some_and(|n| n == "target") && dir.join("CACHEDIR.TAG").is_file()
    })
}

/// `<bundle>.app/Contents/MacOS/<exe>` to `<bundle>.app`.
pub(crate) fn macos_bundle(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let editor = contents.parent()?;
    if macos.file_name()? != "MacOS" || contents.file_name()? != "Contents" {
        return None;
    }
    if editor.file_name()? == "PaintCraft Editor.app" {
        let helpers = editor.parent()?;
        let outer_contents = helpers.parent()?;
        let outer = outer_contents.parent()?;
        (helpers.file_name()? == "Helpers"
            && outer_contents.file_name()? == "Contents"
            && outer.file_name()? == "PaintCraft.app")
            .then(|| outer.to_path_buf())
    } else {
        (editor.file_name()? == "PaintCraft.app").then(|| editor.to_path_buf())
    }
}

fn windows_kind(exe: &Path, program_files: &[PathBuf]) -> Option<InstallKind> {
    let dir = exe.parent()?.to_path_buf();
    let lower = |path: &Path| {
        path.to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    };
    let here = lower(&dir);
    let msi = program_files
        .iter()
        .any(|root| here == format!("{}\\paintcraft", lower(root)));
    Some(if msi {
        InstallKind::WindowsMsi { dir }
    } else {
        InstallKind::WindowsPortable { dir }
    })
}

/// Require the Type2 runtime and its actual mounted PaintCraft executable.
/// Inherited AppImage variables from an unrelated application never select an install.
fn appimage(input: &DetectInput, exe: &Path) -> Option<PathBuf> {
    let image = PathBuf::from(
        input
            .appimage
            .as_deref()
            .filter(|value| !value.is_empty())?,
    );
    if !image.is_absolute() || !std::fs::symlink_metadata(&image).ok()?.is_file() {
        return None;
    }
    let appdir = PathBuf::from(input.appdir.as_deref().filter(|value| !value.is_empty())?);
    if !appdir.is_absolute() {
        return None;
    }
    let appdir = std::fs::canonicalize(appdir).ok()?;
    if !appdir.is_dir() {
        return None;
    }
    let expected = std::fs::canonicalize(appdir.join("usr/bin/paintcraft")).ok()?;
    let actual = std::fs::canonicalize(exe).ok()?;
    if expected != actual || !expected.starts_with(&appdir) {
        return None;
    }
    if !elf_header(&actual, input.arch, ElfKind::Program) {
        return None;
    }
    let image = std::fs::canonicalize(image).ok()?;
    elf_header(&image, input.arch, ElfKind::Type2Runtime).then_some(image)
}
#[derive(Clone, Copy)]
enum ElfKind {
    Program,
    Type2Runtime,
}
fn elf_header(path: &Path, arch: Arch, kind: ElfKind) -> bool {
    use std::io::Read;
    let machine = match arch {
        Arch::X64 => 62,
        Arch::Arm64 => 183,
        Arch::Other => return false,
    };
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    if !file.metadata().is_ok_and(|meta| meta.is_file()) {
        return false;
    }
    let mut header = [0u8; 64];
    if file.read_exact(&mut header).is_err() {
        return false;
    }
    header[..7] == [0x7f, b'E', b'L', b'F', 2, 1, 1]
        && matches!(header[7], 0 | 3)
        && matches!(u16::from_le_bytes([header[16], header[17]]), 2 | 3)
        && u16::from_le_bytes([header[18], header[19]]) == machine
        && (!matches!(kind, ElfKind::Type2Runtime) || header[8..11] == [b'A', b'I', 2])
}
