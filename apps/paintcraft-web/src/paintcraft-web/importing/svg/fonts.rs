use super::*;

#[derive(Debug)]
pub(super) struct FontFile {
    pub(super) name: String,
    pub(super) bytes: Vec<u8>,
}
#[derive(Debug, Default)]
pub(super) struct Inbox {
    pub(super) key: Option<svg_import::FontKey>,
    pub(super) result: Option<Result<Option<Vec<FontFile>>>>,
}
impl Pending {
    fn font_key(&self) -> svg_import::FontKey {
        svg_import::FontKey {
            source: self.id.as_u128(),
            files: self.font_files.as_u128(),
        }
    }
    pub(super) fn sync_fonts(&mut self) {
        self.state.set_font_files(
            self.font_key(),
            self.additional
                .iter()
                .map(|font| font.name.clone())
                .collect(),
            self.font_busy,
        );
    }
    pub(super) fn remove_font(&mut self, index: usize) {
        if !self.font_busy && index < self.additional.len() {
            self.additional.remove(index);
            self.font_files = uuid::Uuid::new_v4();
            self.fonts = None;
            *self.font_inbox.borrow_mut() = Inbox::default();
            self.sync_fonts();
        }
    }
    pub(super) fn build_fonts(&self, additional: &[FontFile]) -> Result<svg::Fonts> {
        let mut fonts = paintcraft_ui::fonts::bundled_data().to_vec();
        fonts.extend(
            self.additional
                .iter()
                .chain(additional)
                .map(|font| font.bytes.as_slice()),
        );
        svg::Fonts::from_data(&fonts, "Selawik", &|| Ok(()))
    }
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn remaining(&self) -> Result<(usize, usize)> {
        let bundled = paintcraft_ui::fonts::bundled_data();
        let count = bundled.len() + self.additional.len();
        let bytes = bundled.iter().map(|font| font.len()).sum::<usize>()
            + self
                .additional
                .iter()
                .map(|font| font.bytes.len())
                .sum::<usize>();
        Ok((
            svg::FONT_FILES_MAX
                .checked_sub(count)
                .filter(|n| *n > 0)
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::ResourceLimit,
                        "SVG font inventory exceeds allowance",
                    )
                })?,
            svg::FONT_BYTES_MAX
                .checked_sub(bytes)
                .filter(|n| *n > 0)
                .ok_or_else(|| {
                    Error::new(ErrorCode::ResourceLimit, "SVG font bytes exceed allowance")
                })?,
        ))
    }
    pub(super) fn accept_fonts(&mut self, ctx: &egui::Context) {
        let result = {
            let mut inbox = self.font_inbox.borrow_mut();
            if inbox.key == Some(self.font_key()) {
                inbox.result.take()
            } else {
                None
            }
        };
        let Some(result) = result else {
            return;
        };
        self.font_busy = false;
        self.font_inbox.borrow_mut().key = None;
        let result = result.and_then(|files| {
            if let Some(files) = files {
                if files.is_empty() {
                    return Ok(());
                }
                if files.iter().any(|font| font.name.len() > 4096) {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "SVG font filename exceeds allowance",
                    ));
                }
                let fonts = self.build_fonts(&files)?;
                self.additional.extend(files);
                self.fonts = Some(fonts);
                self.font_files = uuid::Uuid::new_v4();
            }
            Ok(())
        });
        if let Err(error) = result {
            self.state.font_failure(t!(
                ctx,
                "Could not add font files: { $detail }",
                detail = error
            ));
        }
        self.sync_fonts();
    }
    #[cfg(target_arch = "wasm32")]
    pub(super) fn pick_fonts(&mut self, ctx: &egui::Context) {
        if self.font_busy {
            return;
        }
        let (files_max, bytes_max) = match self.remaining() {
            Ok(limits) => limits,
            Err(error) => {
                self.state.font_failure(t!(
                    ctx,
                    "Could not add font files: { $detail }",
                    detail = error
                ));
                return;
            }
        };
        self.font_busy = true;
        let key = self.font_key();
        *self.font_inbox.borrow_mut() = Inbox {
            key: Some(key),
            result: None,
        };
        let inbox = self.font_inbox.clone();
        let title = t!(ctx, "Add font files");
        let filter = t!(ctx, "Font files");
        let repaint = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = async {
                let Some(files) = rfd::AsyncFileDialog::new()
                    .set_title(title)
                    .add_filter(filter, &["ttf", "otf", "ttc", "otc"])
                    .pick_files()
                    .await
                else {
                    return Ok(None);
                };
                if files.is_empty() {
                    return Ok(None);
                }
                if files.len() > files_max {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "SVG font inventory exceeds allowance",
                    ));
                }
                let mut remaining = bytes_max;
                let mut fonts = Vec::new();
                for file in files {
                    let name = file.file_name();
                    if name.len() > 4096 {
                        return Err(Error::new(
                            ErrorCode::ResourceLimit,
                            "SVG font filename exceeds allowance",
                        ));
                    }
                    if file.inner().size() > remaining as f64 {
                        return Err(Error::new(
                            ErrorCode::ResourceLimit,
                            "SVG font bytes exceed allowance",
                        ));
                    }
                    let buffer = wasm_bindgen_futures::JsFuture::from(file.inner().array_buffer())
                        .await
                        .map_err(|_| {
                            Error::new(ErrorCode::StorageFailed, "Browser font read failed")
                        })?;
                    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                    remaining = remaining.checked_sub(bytes.len()).ok_or_else(|| {
                        Error::new(ErrorCode::ResourceLimit, "SVG font bytes exceed allowance")
                    })?;
                    fonts.push(FontFile { name, bytes });
                }
                Ok(Some(fonts))
            }
            .await;
            if inbox.borrow().key == Some(key) {
                inbox.borrow_mut().result = Some(result);
            }
            repaint.request_repaint();
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn pick_fonts(&mut self, ctx: &egui::Context) {
        self.state.font_failure(t!(
            ctx,
            "Could not add font files: { $detail }",
            detail = "File picking is available in the browser build"
        ));
    }
}
#[cfg(test)]
mod tests;
