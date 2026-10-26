use super::*;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};
#[derive(Clone, Copy, Debug)]
pub(super) enum Target {
    MacArm,
    MacX64,
    WinArm,
    WinX64,
    LinuxArm,
    LinuxX64,
}
impl Target {
    pub fn current() -> Result<Self> {
        let environment = if cfg!(target_env = "gnu") {
            "gnu"
        } else if cfg!(target_env = "msvc") {
            "msvc"
        } else {
            ""
        };
        Self::from_environment(std::env::consts::OS, std::env::consts::ARCH, environment)
    }
    pub(super) fn from_environment(
        os: &str,
        architecture: &str,
        environment: &str,
    ) -> Result<Self> {
        Ok(match (os, architecture, environment) {
            ("macos", "aarch64", "") => Self::MacArm,
            ("macos", "x86_64", "") => Self::MacX64,
            ("windows", "aarch64", "msvc") => Self::WinArm,
            ("windows", "x86_64", "msvc") => Self::WinX64,
            ("linux", "aarch64", "gnu") => Self::LinuxArm,
            ("linux", "x86_64", "gnu") => Self::LinuxX64,
            _ => {
                return Err(unavailable(
                    "Native helper compiled target/environment is not supported",
                ));
            }
        })
    }
    pub fn triple(self) -> &'static str {
        match self {
            Self::MacArm => "aarch64-apple-darwin",
            Self::MacX64 => "x86_64-apple-darwin",
            Self::WinArm => "aarch64-pc-windows-msvc",
            Self::WinX64 => "x86_64-pc-windows-msvc",
            Self::LinuxArm => "aarch64-unknown-linux-gnu",
            Self::LinuxX64 => "x86_64-unknown-linux-gnu",
        }
    }
    pub fn executable(self) -> &'static str {
        if matches!(self, Self::WinArm | Self::WinX64) {
            "paintcraft-codec-worker.exe"
        } else {
            "paintcraft-codec-worker"
        }
    }
    pub fn linux(self) -> bool {
        matches!(self, Self::LinuxArm | Self::LinuxX64)
    }
    pub fn verify(self, mut file: &File) -> Result<()> {
        file.seek(SeekFrom::Start(0)).map_err(storage)?;
        let mut head = [0; 64];
        file.read_exact(&mut head)
            .map_err(|_| unavailable("Truncated helper executable header"))?;
        let valid = match self {
            Self::MacArm | Self::MacX64 => {
                head[..4] == 0xfeedfacfu32.to_le_bytes()
                    && u32::from_le_bytes(head[4..8].try_into().unwrap())
                        == if matches!(self, Self::MacArm) {
                            0x0100000c
                        } else {
                            0x01000007
                        }
                    && u32::from_le_bytes(head[12..16].try_into().unwrap()) == 2
            }
            Self::WinArm | Self::WinX64 => {
                if head[..2] != *b"MZ" {
                    false
                } else {
                    let offset = u64::from(u32::from_le_bytes(head[60..64].try_into().unwrap()));
                    if offset > 1 << 20
                        || offset
                            .checked_add(26)
                            .is_none_or(|end| file.metadata().map_or(true, |m| end > m.len()))
                    {
                        false
                    } else {
                        file.seek(SeekFrom::Start(offset)).map_err(storage)?;
                        let mut pe = [0; 26];
                        file.read_exact(&mut pe).map_err(storage)?;
                        let flags = u16::from_le_bytes(pe[22..24].try_into().unwrap());
                        pe[..4] == *b"PE\0\0"
                            && u16::from_le_bytes(pe[4..6].try_into().unwrap())
                                == if matches!(self, Self::WinArm) {
                                    0xaa64
                                } else {
                                    0x8664
                                }
                            && u16::from_le_bytes(pe[24..26].try_into().unwrap()) == 0x20b
                            && flags & 2 != 0
                            && flags & 0x2000 == 0
                    }
                }
            }
            Self::LinuxArm | Self::LinuxX64 => {
                head[..7] == [0x7f, b'E', b'L', b'F', 2, 1, 1]
                    && matches!(head[7], 0 | 3)
                    && matches!(u16::from_le_bytes(head[16..18].try_into().unwrap()), 2 | 3)
                    && u16::from_le_bytes(head[18..20].try_into().unwrap())
                        == if matches!(self, Self::LinuxArm) {
                            183
                        } else {
                            62
                        }
            }
        };
        if valid {
            Ok(())
        } else {
            Err(unavailable(
                "Installed helper executable target/architecture mismatch",
            ))
        }
    }
}
