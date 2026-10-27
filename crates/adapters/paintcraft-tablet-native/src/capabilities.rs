//! Installation and native advertisements are not physical qualification.
use paintcraft_input::tablet::DeviceKind;
#[cfg(any(target_os = "macos", test))]
use paintcraft_input::tablet::RawSample;
#[cfg(any(target_os = "macos", test))]
use std::sync::{Arc, Mutex};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    AppKit,
    WindowsPointer,
    XInput2,
    WaylandTabletV2,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Axes {
    pub pressure: bool,
    pub tilt: bool,
    pub twist: bool,
    pub contact: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub id: u64,
    pub kind: DeviceKind,
    pub advertised: Axes,
    pub observed: Axes,
    pub native_capabilities: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilitySnapshot {
    pub backend: Backend,
    pub installed: bool,
    pub devices: Vec<Device>,
    pub samples: u64,
    pub callback_failures: u64,
    pub sink_failures: u64,
    pub source_failures: u64,
    pub wake_failures: u64,
    pub unclassified_points: u64,
    pub rejected_devices: u64,
}
#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Debug)]
pub(crate) struct Report(Arc<Mutex<CapabilitySnapshot>>);
#[cfg(any(target_os = "macos", test))]
impl Report {
    pub(crate) fn new(backend: Backend) -> Self {
        Self(Arc::new(Mutex::new(CapabilitySnapshot {
            backend,
            installed: false,
            devices: Vec::new(),
            samples: 0,
            callback_failures: 0,
            sink_failures: 0,
            source_failures: 0,
            wake_failures: 0,
            unclassified_points: 0,
            rejected_devices: 0,
        })))
    }
    pub(crate) fn snapshot(&self) -> CapabilitySnapshot {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    #[cfg(target_os = "macos")]
    pub(crate) fn installed(&self, value: bool) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).installed = value;
    }
    pub(crate) fn device(&self, device: Device) -> bool {
        let mut report = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(old) = report.devices.iter_mut().find(|old| old.id == device.id) {
            let observed = old.observed;
            *old = device;
            old.observed = observed;
        } else if report.devices.len() < 64 && report.devices.try_reserve(1).is_ok() {
            report.devices.push(device);
        } else {
            report.rejected_devices = report.rejected_devices.saturating_add(1);
            return false;
        }
        true
    }
    pub(crate) fn sample(&self, raw: RawSample) {
        let mut report = self.0.lock().unwrap_or_else(|e| e.into_inner());
        report.samples = report.samples.saturating_add(1);
        if let Some(device) = report
            .devices
            .iter_mut()
            .find(|d| d.id == raw.identity.device)
        {
            device.observed.pressure |= raw.pressure.is_some();
            device.observed.tilt |= raw.tilt.is_some();
            device.observed.twist |= raw.twist_degrees.is_some();
            device.observed.contact |= raw.contact.is_some();
        }
    }
    pub(crate) fn wake_failed(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .wake_failures
            != 0
    }
    #[cfg(target_os = "macos")]
    pub(crate) fn unclassified(&self) {
        let mut r = self.0.lock().unwrap_or_else(|e| e.into_inner());
        r.unclassified_points = r.unclassified_points.saturating_add(1);
    }
    pub(crate) fn sink_failure(&self) {
        let mut r = self.0.lock().unwrap_or_else(|e| e.into_inner());
        r.sink_failures = r.sink_failures.saturating_add(1);
        r.callback_failures = r.callback_failures.saturating_add(1);
    }
    pub(crate) fn source_failure(&self) {
        let mut r = self.0.lock().unwrap_or_else(|e| e.into_inner());
        r.source_failures = r.source_failures.saturating_add(1);
        r.callback_failures = r.callback_failures.saturating_add(1);
    }
    pub(crate) fn wake_failure(&self) {
        let mut r = self.0.lock().unwrap_or_else(|e| e.into_inner());
        r.wake_failures = r.wake_failures.saturating_add(1);
        r.callback_failures = r.callback_failures.saturating_add(1);
    }
}
