//! One exclusive IMM reader. Every buffer is copied/validated before releasing
//! the input context, and the context is released before any external Sink.
use super::*;
/// Primitive storage writable by IMM.
///
/// # Safety
/// Every possible bit pattern must be valid and values must need no destructor.
unsafe trait ImmWord: Copy {}
unsafe impl ImmWord for u8 {}
unsafe impl ImmWord for u16 {}
unsafe impl ImmWord for u32 {}
struct Lease {
    hwnd: HWND,
    context: Option<HIMC>,
}
impl Lease {
    fn new(hwnd: HWND) -> Result<Self> {
        let context = unsafe { ImmGetContext(hwnd) };
        if context.0.is_null() {
            return Err(error("Native IME context unavailable"));
        }
        Ok(Self {
            hwnd,
            context: Some(context),
        })
    }
    fn raw(&self) -> HIMC {
        self.context.expect("live private IME lease")
    }
    fn count(&self, field: IME_COMPOSITION_STRING, width: usize) -> Result<usize> {
        let bytes = unsafe { ImmGetCompositionStringW(self.raw(), field, None, 0) };
        if bytes < 0 {
            return Err(error("Native IME snapshot data unavailable"));
        }
        let bytes = bytes as usize;
        if !bytes.is_multiple_of(width) || bytes / width > MAX_CALLBACK_RECORDS {
            return Err(error("Native IME snapshot size invalid"));
        }
        Ok(bytes / width)
    }
    fn copy<T: ImmWord>(&self, field: IME_COMPOSITION_STRING, out: &mut [T]) -> Result<()> {
        if out.is_empty() {
            return Ok(());
        }
        let bytes: u32 = size_of_val(out)
            .try_into()
            .map_err(|_| error("Native IME buffer overflow"))?;
        // SAFETY: initialized caller-owned admitted buffer of exactly bytes;
        // query/fetch have no intervening message retrieval or external callback.
        let copied = unsafe {
            ImmGetCompositionStringW(self.raw(), field, Some(out.as_mut_ptr().cast()), bytes)
        };
        if copied < 0 || copied as u32 != bytes {
            return Err(error("Native IME snapshot changed or truncated"));
        }
        Ok(())
    }
    fn cursor(&self) -> Result<Option<u32>> {
        match unsafe { ImmGetCompositionStringW(self.raw(), GCS_CURSORPOS, None, 0) } {
            -1 => Ok(None), // IMM_ERROR_NODATA, distinct from a fabricated caret.
            value if value < 0 => Err(error("Native IME cursor query failed")),
            value => Ok(Some(value as u32)),
        }
    }
    fn release(mut self) -> Result<()> {
        let context = self.context.take().expect("live private IME lease");
        if !unsafe { ImmReleaseContext(self.hwnd, context) }.as_bool() {
            return Err(error("Native IME context release failed"));
        }
        Ok(())
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(context) = self.context.take() {
            let _ = unsafe { ImmReleaseContext(self.hwnd, context) };
        }
    }
}
pub(super) fn acquire(capture: &mut Capture, hwnd: HWND, flags: u32) -> Result<()> {
    let composition = capture.state.composition().map_err(fault)?;
    if flags & (CS_INSERTCHAR | CS_NOMOVECARET) != 0 {
        return Err(error("Native IME insertion protocol unsupported"));
    }
    let gcs = GCS_COMPREADSTR.0
        | GCS_COMPREADATTR.0
        | GCS_COMPREADCLAUSE.0
        | GCS_COMPSTR.0
        | GCS_COMPATTR.0
        | GCS_COMPCLAUSE.0
        | GCS_CURSORPOS.0
        | GCS_DELTASTART.0
        | GCS_RESULTREADSTR.0
        | GCS_RESULTREADCLAUSE.0
        | GCS_RESULTSTR.0
        | GCS_RESULTCLAUSE.0;
    if flags & gcs == 0 {
        return capture.push(NativeUi::ImeNativeCancel { composition });
    }
    let result = flags & GCS_RESULTSTR.0 != 0;
    let result_clauses = flags & GCS_RESULTCLAUSE.0 != 0;
    let preedit = flags
        & (GCS_COMPSTR.0 | GCS_COMPATTR.0 | GCS_COMPCLAUSE.0 | GCS_CURSORPOS.0 | GCS_DELTASTART.0)
        != 0;
    let attributes = flags & GCS_COMPATTR.0 != 0;
    let preedit_clauses = flags & GCS_COMPCLAUSE.0 != 0;
    if !result && !result_clauses && !preedit {
        return Err(error("Native IME reading-only protocol unsupported"));
    }
    let lease = Lease::new(hwnd)?;
    let rn = if result || result_clauses {
        lease.count(GCS_RESULTSTR, 2)?
    } else {
        0
    };
    let pn = if preedit {
        lease.count(GCS_COMPSTR, 2)?
    } else {
        0
    };
    let an = if attributes {
        lease.count(GCS_COMPATTR, 1)?
    } else {
        0
    };
    let pcn = if preedit_clauses {
        lease.count(GCS_COMPCLAUSE, 4)?
    } else {
        0
    };
    let rcn = if result_clauses {
        lease.count(GCS_RESULTCLAUSE, 4)?
    } else {
        0
    };
    // Existing control + possible observed idle are part of the 4096-record
    // Collector/Projection limit. Every actual component needs two envelopes.
    let mut total = 2usize;
    for (included, count) in [
        (result, rn),
        (preedit, pn),
        (attributes, an),
        (preedit_clauses, pcn),
        (result_clauses, rcn),
    ] {
        if included {
            total = total
                .checked_add(count)
                .and_then(|n| n.checked_add(2))
                .ok_or_else(|| error("Native IME record overflow"))?;
        }
    }
    if total > MAX_CALLBACK_RECORDS {
        return Err(error("Native IME callback admission refused"));
    }
    lease.copy(GCS_RESULTSTR, &mut capture.result[..rn])?;
    lease.copy(GCS_COMPSTR, &mut capture.preedit[..pn])?;
    lease.copy(GCS_COMPATTR, &mut capture.attributes[..an])?;
    lease.copy(GCS_COMPCLAUSE, &mut capture.preedit_clauses[..pcn])?;
    lease.copy(GCS_RESULTCLAUSE, &mut capture.result_clauses[..rcn])?;
    let cursor = if flags & GCS_CURSORPOS.0 != 0 {
        lease.cursor()?
    } else {
        None
    };
    validate_snapshot(
        &capture.result[..rn],
        &capture.preedit[..pn],
        &capture.attributes[..an],
        &capture.preedit_clauses[..pcn],
        &capture.result_clauses[..rcn],
    )
    .map_err(fault)?;
    lease.release()?;
    fn frame<T: Copy + Into<u32>>(
        records: &mut Vec<NativeUi>,
        composition: u64,
        component: ImeComponent,
        data: &[T],
        cursor_native: Option<u32>,
        native_flags: u32,
    ) {
        records.push(NativeUi::ImeFrameStart {
            composition,
            component,
            count: data.len() as u16,
            cursor_native,
            native_flags,
        });
        records.extend(
            data.iter()
                .enumerate()
                .map(|(index, value)| NativeUi::ImeData {
                    index: index as u16,
                    value: (*value).into(),
                }),
        );
        records.push(NativeUi::ImeFrameEnd { composition });
    }
    if result {
        frame(
            &mut capture.records,
            composition,
            ImeComponent::ResultUtf16,
            &capture.result[..rn],
            None,
            flags,
        );
    }
    if preedit {
        frame(
            &mut capture.records,
            composition,
            ImeComponent::PreeditUtf16,
            &capture.preedit[..pn],
            cursor,
            flags,
        );
    }
    if attributes {
        frame(
            &mut capture.records,
            composition,
            ImeComponent::WindowsAttributes,
            &capture.attributes[..an],
            None,
            flags,
        );
    }
    if preedit_clauses {
        frame(
            &mut capture.records,
            composition,
            ImeComponent::WindowsPreeditClauses,
            &capture.preedit_clauses[..pcn],
            None,
            flags,
        );
    }
    if result_clauses {
        frame(
            &mut capture.records,
            composition,
            ImeComponent::WindowsResultClauses,
            &capture.result_clauses[..rcn],
            None,
            flags,
        );
    }
    Ok(())
}
