use super::*;
pub(super) struct Cursor<'w, 'm> {
    pub input: Input<'w>,
    pub meter: &'m Meter<'m>,
}
impl<'w, 'm> Cursor<'w, 'm> {
    pub fn new(bytes: &'w [u8], meter: &'m Meter<'m>) -> Self {
        Self {
            input: Input::new(bytes),
            meter,
        }
    }
    pub fn take(&mut self, n: usize) -> Result<&'w [u8]> {
        self.meter.step(1)?;
        self.input.take(n)
    }
    pub fn byte(&mut self) -> Result<u8> {
        self.meter.step(1)?;
        self.input.byte()
    }
    pub fn number(&mut self, max: u64) -> Result<u64> {
        self.meter.step(10)?;
        self.input.number(max)
    }
    pub fn signed(&mut self) -> Result<i64> {
        self.meter.step(10)?;
        self.input.signed()
    }
    pub fn dimension(&mut self) -> Result<u32> {
        self.meter.step(10)?;
        self.input.dimension()
    }
    pub fn id(&mut self) -> Result<[u8; 16]> {
        Ok(self.take(16)?.try_into().unwrap())
    }
    pub fn flag(&mut self) -> Result<bool> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(invalid()),
        }
    }
    pub fn text(&mut self, bytes: usize, chars: usize) -> Result<&'w str> {
        let len = self.number(bytes as u64)? as usize;
        self.meter
            .step((len as u64).checked_mul(2).ok_or_else(bound)?)?;
        let s = std::str::from_utf8(self.input.take(len)?).map_err(|_| invalid())?;
        if s.chars().count() > chars {
            return Err(invalid());
        }
        Ok(s)
    }
    fn descriptor_pair(&mut self, catalog: &Catalog) -> Result<(usize, Descriptor)> {
        let index = self.number(255)? as usize;
        let len = self.number(65536)? as usize;
        if len > 6 {
            return Err(invalid());
        }
        let bytes = self.take(len)?;
        self.meter.step(33 + len as u64)?;
        let reg = catalog
            .entries
            .get(index)
            .filter(|_| index < catalog.len)
            .and_then(|r| *r)
            .ok_or_else(invalid)?;
        Ok((index, reg.decode(bytes)?))
    }
    pub fn descriptor(&mut self, catalog: &mut Catalog) -> Result<Descriptor> {
        let (i, d) = self.descriptor_pair(catalog)?;
        catalog.used[i] = true;
        Ok(d)
    }
    pub fn descriptor_view(&mut self, catalog: &Catalog) -> Result<Descriptor> {
        self.descriptor_pair(catalog).map(|(_, d)| d)
    }
    pub fn catalog(&mut self) -> Result<Catalog> {
        let len = self.number(65536)? as usize;
        let bytes = self.take(len)?;
        let mut c = Cursor::new(bytes, self.meter);
        let count = c.number(256)? as usize;
        c.meter.step(size_of::<Catalog>() as u64)?;
        let mut out = Catalog::new();
        let mut prior: Option<&str> = None;
        for i in 0..count {
            let profile = c.text(96, 96)?;
            let codec = c.text(64, 64)?;
            c.meter
                .step(2080 + 3 * (profile.len() + codec.len()) as u64)?; // registry + identifiers/order
            identifier(profile)?;
            identifier(codec)?;
            if prior.is_some_and(|p| p >= profile) {
                return Err(invalid());
            }
            let reg = resolve_for(RegistryContract::LayerAppearance4, profile, codec)?;
            out.entries[i] = Some(reg);
            out.len += 1;
            prior = Some(profile);
        }
        if c.input.remaining() != 0 {
            return Err(invalid());
        }
        Ok(out)
    }
    pub fn mask(
        &mut self,
        limits: &mut paintcraft_raster::coverage_mask::Limits,
    ) -> Result<Option<PortableMaskView<'w>>> {
        limits.work = limits.work.checked_sub(1).ok_or_else(bound)?;
        let start = self.input.rest();
        if !self.flag()? {
            return Ok(None);
        }
        let enabled = self.flag()?;
        if self.byte()? != 1 {
            return Err(unsupported("unknown portable A8 codec"));
        }
        let (x0, y0, x1, y1) = (
            self.signed()?,
            self.signed()?,
            self.signed()?,
            self.signed()?,
        );
        let domain = Rect::new(
            x0,
            y0,
            x1.checked_sub(x0).ok_or_else(invalid)?,
            y1.checked_sub(y0).ok_or_else(invalid)?,
        )
        .ok_or_else(invalid)?;
        let bands = usize::try_from(self.number(u64::MAX)?).map_err(|_| bound())?;
        if bands > limits.bands {
            return Err(bound());
        }
        let mut runs = 0usize;
        let mut last_y = y0;
        let mut previous: Option<(i64, &[u8])> = None;
        for _ in 0..bands {
            // Existing canonical masks charge named band/run work too.
            limits.work = limits.work.checked_sub(1).ok_or_else(bound)?;
            let y = add_signed(y0, self.number(domain.height() as u64)?)?;
            let h = self.number(domain.height() as u64)?;
            if h == 0 {
                return Err(invalid());
            }
            let end = add_signed(y, h)?;
            if y < last_y || end > y1 {
                return Err(invalid());
            }
            let n = usize::try_from(self.number(u64::MAX)?).map_err(|_| bound())?;
            if n == 0 {
                return Err(invalid());
            }
            runs = runs
                .checked_add(n)
                .filter(|n| *n <= limits.runs)
                .ok_or_else(bound)?;
            let row = self.input.rest();
            let mut last_x = x0;
            let mut last_a = 0;
            for _ in 0..n {
                limits.work = limits.work.checked_sub(1).ok_or_else(bound)?;
                let x = add_signed(x0, self.number(domain.width() as u64)?)?;
                let w = self.number(domain.width() as u64)?;
                if w == 0 {
                    return Err(invalid());
                }
                let end = add_signed(x, w)?;
                let a = self.byte()?;
                if a == 0 || x < last_x || end > x1 || (x == last_x && a == last_a) {
                    return Err(invalid());
                }
                last_x = end;
                last_a = a;
            }
            let row = &row[..row.len() - self.input.remaining()];
            self.meter.step(row.len() as u64)?; // before bounded adjacent-row equality
            if previous.is_some_and(|(end, p)| end == y && p == row) {
                return Err(invalid());
            }
            previous = Some((end, row));
            last_y = end;
        }
        limits.bands -= bands;
        limits.runs -= runs;
        let bytes = &start[..start.len() - self.input.remaining()];
        Ok(Some(PortableMaskView {
            domain,
            enabled,
            bands: bands as u64,
            runs: runs as u64,
            bytes,
        }))
    }
}
fn add_signed(a: i64, b: u64) -> Result<i64> {
    a.checked_add(i64::try_from(b).map_err(|_| invalid())?)
        .ok_or_else(invalid)
}

fn identifier(value: &str) -> Result<()> {
    let (name, version) = value.rsplit_once("-v").ok_or_else(invalid)?;
    if !name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || version.is_empty()
        || version.starts_with('0')
        || !version.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(invalid());
    }
    Ok(())
}
