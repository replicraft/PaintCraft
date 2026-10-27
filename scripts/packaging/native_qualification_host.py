"""Native runner identity: refuse a cross-target or translated runtime claim."""
import ctypes
import json
import os
import platform
import sys


TARGETS = {
    ("Windows", "x86_64"): "x86_64-pc-windows-msvc",
    ("Windows", "aarch64"): "aarch64-pc-windows-msvc",
    ("Darwin", "x86_64"): "x86_64-apple-darwin",
    ("Darwin", "aarch64"): "aarch64-apple-darwin",
    ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
    ("Linux", "aarch64"): "aarch64-unknown-linux-gnu",
}


def normalize(machine):
    return {"arm64": "aarch64", "aarch64": "aarch64", "amd64": "x86_64",
            "x86_64": "x86_64"}.get(machine.lower())


def windows_native_machine():
    # Python may itself run under emulation; qualify the native Rust executables,
    # not Python's process architecture or an inherited environment claim.
    from ctypes import wintypes
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.GetCurrentProcess.restype = wintypes.HANDLE
    kernel.IsWow64Process2.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.USHORT),
                                     ctypes.POINTER(wintypes.USHORT)]
    kernel.IsWow64Process2.restype = wintypes.BOOL
    process, native = wintypes.USHORT(), wintypes.USHORT()
    if not kernel.IsWow64Process2(kernel.GetCurrentProcess(), ctypes.byref(process), ctypes.byref(native)):
        raise OSError(ctypes.get_last_error(), "IsWow64Process2 failed")
    return {0x8664: "x86_64", 0xAA64: "aarch64"}.get(native.value)


def validate(target, rustc, *, system, machine, translated=False, native_windows=None):
    hosts = [line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: ")]
    architecture = native_windows if system == "Windows" else normalize(machine)
    actual = TARGETS.get((system, architecture))
    if translated or len(hosts) != 1 or hosts[0] != target or actual != target:
        raise ValueError("Native qualification requires matching OS/native machine/Rust host; no emulation or cross-target claim")
    return {"system": system, "python_machine": machine, "native_machine": architecture,
            "rust_host": hosts[0], "target": actual, "translated": translated,
            "python_version": sys.version, "macos_deployment_target": os.environ.get("MACOSX_DEPLOYMENT_TARGET")}


def identify(target, runner, root, env):
    rust = runner.run("rustc", ["rustc", "--version", "--verbose"], cwd=root, env=env, timeout=30).read_text()
    system = platform.system()
    observed = {"system": system, "machine": platform.machine(), "translated": False,
                "native_windows": None, "rustc_verbose": rust}
    try:
        if system == "Darwin":
            stdout = runner.run("rosetta", ["/usr/sbin/sysctl", "-n", "sysctl.proc_translated"],
                                cwd=root, env=env, timeout=10, allow_failure=True)
            result = runner.steps[-1]
            if result["exit"] == 0:
                value = stdout.read_text().strip()
                if value not in ("0", "1"):
                    raise ValueError("Unexpected native translation probe response")
                observed["translated"] = value == "1"
        if system == "Windows":
            observed["native_windows"] = windows_native_machine()
    finally:
        (runner.output / "native-host.json").write_text(json.dumps(observed, indent=2) + "\n", encoding="utf-8")
    return validate(target, rust, system=system, machine=observed["machine"],
                    translated=observed["translated"], native_windows=observed["native_windows"])
