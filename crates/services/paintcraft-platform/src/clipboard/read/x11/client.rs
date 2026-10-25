//! A fresh connection owns all invisible requestor windows until the read ends.
use super::transfer::{CHUNK, Step, Transfer};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use std::{
    fmt::Display,
    time::{Duration, Instant},
};
use x11rb::{
    connection::Connection,
    protocol::{Event, xproto::*},
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};
pub(super) fn failure(error: impl Display) -> String {
    format!("X11 clipboard: {error}")
}
pub(super) struct Atoms {
    pub selection: u32,
    pub targets: u32,
    pub timestamp: u32,
    pub png: u32,
    pub tiff: u32,
    property: u32,
    incr: u32,
}
pub(super) struct Client {
    conn: RustConnection,
    root: Window,
    pub atoms: Atoms,
    owner: Window,
}
pub(super) struct Reading {
    window: Window,
    transfer: Transfer,
}
impl Client {
    pub fn connect(display: Option<&str>, selection: &[u8]) -> Result<Self, String> {
        let (conn, screen) = x11rb::connect(display).map_err(failure)?;
        let root = conn
            .setup()
            .roots
            .get(screen)
            .ok_or("Invalid X11 screen")?
            .root;
        let intern = |name: &[u8]| -> Result<u32, String> {
            Ok(conn
                .intern_atom(false, name)
                .map_err(failure)?
                .reply()
                .map_err(failure)?
                .atom)
        };
        let atoms = Atoms {
            selection: intern(selection)?,
            targets: intern(b"TARGETS")?,
            timestamp: intern(b"TIMESTAMP")?,
            png: intern(b"image/png")?,
            tiff: intern(b"image/tiff")?,
            property: intern(b"_PAINTCRAFT_CLIPBOARD_READ")?,
            incr: intern(b"INCR")?,
        };
        let owner = conn
            .get_selection_owner(atoms.selection)
            .map_err(failure)?
            .reply()
            .map_err(failure)?
            .owner;
        if owner == x11rb::NONE {
            return Err("X11 clipboard has no owner".into());
        }
        Ok(Self {
            conn,
            root,
            atoms,
            owner,
        })
    }
    pub fn same_owner(&self, check: &dyn Fn() -> Result<(), String>) -> Result<(), String> {
        check()?;
        let owner = self
            .conn
            .get_selection_owner(self.atoms.selection)
            .map_err(failure)?
            .reply()
            .map_err(failure)?
            .owner;
        check()?;
        if owner != self.owner {
            return Err("X11 clipboard ownership changed during reading".into());
        }
        Ok(())
    }
    fn wait(
        &self,
        check: &dyn Fn() -> Result<(), String>,
        matches: impl Fn(&Event) -> bool,
    ) -> Result<Event, String> {
        let start = Instant::now();
        loop {
            check()?;
            if start.elapsed() >= Duration::from_secs(30) {
                return Err("X11 clipboard transfer timed out".into());
            }
            if let Some(event) = self.conn.poll_for_event().map_err(failure)? {
                if let Event::Error(error) = event {
                    return Err(format!("X11 clipboard protocol error: {error:?}"));
                }
                if matches(&event) {
                    return Ok(event);
                }
                continue;
            }
            let mut fds = [PollFd::new(self.conn.stream(), PollFlags::IN)];
            let timeout = Timespec {
                tv_sec: 0,
                tv_nsec: 50_000_000,
            };
            match poll(&mut fds, Some(&timeout)) {
                Ok(_)
                    if fds[0]
                        .revents()
                        .intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL) =>
                {
                    return Err("X11 clipboard connection closed".into());
                }
                Ok(_) | Err(rustix::io::Errno::INTR) => {}
                Err(error) => return Err(failure(error)),
            }
        }
    }
    fn property(&self, window: Window, offset: u32) -> Result<GetPropertyReply, String> {
        // X11 offsets/lengths count 32-bit units. Deletion only happens on the
        // final partial read, which also acknowledges an entire INCR property.
        self.conn
            .get_property(
                true,
                window,
                self.atoms.property,
                AtomEnum::ANY,
                offset,
                (CHUNK / 4) as u32,
            )
            .map_err(failure)?
            .reply()
            .map_err(failure)
    }
    pub fn start(
        &self,
        target: Atom,
        expected: Atom,
        format: u8,
        maximum: usize,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<Reading, String> {
        self.same_owner(check)?;
        let window = self.conn.generate_id().map_err(failure)?;
        self.conn
            .create_window(
                0,
                window,
                self.root,
                0,
                0,
                1,
                1,
                0,
                WindowClass::INPUT_ONLY,
                0,
                &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
            )
            .map_err(failure)?
            .check()
            .map_err(failure)?;
        // Obtain a fresh server timestamp without using CurrentTime. Every
        // conversion gets its own window, including repeated TIMESTAMP queries.
        self.conn
            .change_property8(
                PropMode::REPLACE,
                window,
                self.atoms.property,
                AtomEnum::STRING,
                &[],
            )
            .map_err(failure)?;
        self.conn.flush().map_err(failure)?;
        let Event::PropertyNotify(event) = self.wait(check, |event| {
            matches!(event,
            Event::PropertyNotify(e) if e.window == window && e.atom == self.atoms.property
                && e.state == Property::NEW_VALUE)
        })?
        else {
            unreachable!()
        };
        let time = event.time;
        if time == x11rb::CURRENT_TIME {
            return Err("Invalid X11 clipboard request timestamp".into());
        }
        self.conn
            .delete_property(window, self.atoms.property)
            .map_err(failure)?;
        self.conn
            .convert_selection(
                window,
                self.atoms.selection,
                target,
                self.atoms.property,
                time,
            )
            .map_err(failure)?;
        self.conn.flush().map_err(failure)?;
        let Event::SelectionNotify(event) = self.wait(check, |event| matches!(event,
            Event::SelectionNotify(e) if e.requestor == window && e.selection == self.atoms.selection
                && e.target == target && e.time == time))? else { unreachable!() };
        if event.property != self.atoms.property {
            return Err("X11 clipboard conversion was refused or changed its property".into());
        }
        check()?;
        let first = self.property(window, 0)?;
        check()?;
        let transfer = Transfer::new(first, self.atoms.incr, expected, format, maximum)?;
        Ok(Reading { window, transfer })
    }
    pub fn next(
        &self,
        reading: &mut Reading,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<Option<Vec<u8>>, String> {
        loop {
            check()?;
            match reading.transfer.next() {
                Step::Bytes(bytes) => return Ok(Some(bytes)),
                Step::Done => return Ok(None),
                Step::Fetch { offset, wait } => {
                    if wait {
                        self.wait(check, |event| {
                            matches!(event, Event::PropertyNotify(e)
                            if e.window == reading.window && e.atom == self.atoms.property
                                && e.state == Property::NEW_VALUE)
                        })?;
                    }
                    let reply = self.property(reading.window, offset)?;
                    check()?;
                    reading.transfer.accept(reply)?;
                    // The next owner chunk may wait on our deletion acknowledgment.
                    self.conn.flush().map_err(failure)?;
                }
            }
        }
    }
    pub fn words(
        &self,
        target: Atom,
        expected: Atom,
        limit: usize,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<Vec<u32>, String> {
        let mut reading = self.start(target, expected, 32, limit * 4, check)?;
        let mut words = Vec::new();
        while let Some(bytes) = self.next(&mut reading, check)? {
            words
                .try_reserve_exact(bytes.len() / 4)
                .map_err(|_| "Cannot allocate X11 clipboard metadata")?;
            words.extend(
                bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|part| u32::from_ne_bytes(*part)),
            );
        }
        self.same_owner(check)?;
        Ok(words)
    }
}
