//! ICCCM property/INCR validation, independent of connection and event waiting.
use x11rb::protocol::xproto::GetPropertyReply;
pub(super) const CHUNK: usize = 32 * 1024;
pub(super) enum Step {
    Bytes(Vec<u8>),
    Fetch { offset: u32, wait: bool },
    Done,
}
pub(super) struct Transfer {
    expected: u32,
    format: u8,
    maximum: usize,
    total: usize,
    lower_bound: usize,
    incremental: bool,
    offset: u32,
    left: Option<u32>,
    pending: Option<Vec<u8>>,
    done: bool,
}
impl Transfer {
    pub fn new(
        first: GetPropertyReply,
        incr: u32,
        expected: u32,
        format: u8,
        maximum: usize,
    ) -> Result<Self, String> {
        let mut value = Self {
            expected,
            format,
            maximum,
            total: 0,
            lower_bound: 0,
            incremental: first.type_ == incr,
            offset: 0,
            left: None,
            pending: None,
            done: false,
        };
        if value.incremental {
            if first.format != 32
                || first.value_len != 1
                || first.value.len() != 4
                || first.bytes_after != 0
            {
                return Err("Invalid X11 INCR header".into());
            }
            value.lower_bound = first.value32().unwrap().next().unwrap() as usize;
            if value.lower_bound > maximum {
                return Err("X11 clipboard transfer exceeds the limit".into());
            }
        } else {
            value.accept(first)?;
        }
        Ok(value)
    }
    pub fn next(&mut self) -> Step {
        if let Some(bytes) = self.pending.take() {
            Step::Bytes(bytes)
        } else if self.done {
            Step::Done
        } else {
            Step::Fetch {
                offset: self.offset,
                wait: self.left.is_none(),
            }
        }
    }
    pub fn accept(&mut self, reply: GetPropertyReply) -> Result<(), String> {
        if self.done || self.pending.is_some() {
            return Err("Unexpected X11 clipboard property".into());
        }
        let bytes = reply.value.len();
        if reply.type_ != self.expected
            || reply.format != self.format
            || u64::from(reply.value_len) * u64::from(self.format / 8) != bytes as u64
            || bytes > CHUNK
        {
            return Err("X11 clipboard property type or size changed".into());
        }
        let extent = bytes as u64 + u64::from(reply.bytes_after);
        if self.left.is_some_and(|left| extent != u64::from(left)) {
            return Err("X11 clipboard property changed during partial reading".into());
        }
        if extent > (self.maximum - self.total) as u64 {
            return Err("X11 clipboard transfer exceeds the limit".into());
        }
        if reply.bytes_after != 0 {
            if bytes == 0 || !bytes.is_multiple_of(4) {
                return Err("Invalid partial X11 clipboard property".into());
            }
            self.offset = self
                .offset
                .checked_add((bytes / 4) as u32)
                .ok_or("X11 clipboard property offset overflow")?;
            self.left = Some(reply.bytes_after);
        } else {
            self.offset = 0;
            self.left = None;
            self.done = !self.incremental || bytes == 0;
        }
        self.total += bytes;
        if self.done && self.total < self.lower_bound {
            return Err("X11 clipboard transfer ended before its lower bound".into());
        }
        if bytes != 0 {
            self.pending = Some(reply.value);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
