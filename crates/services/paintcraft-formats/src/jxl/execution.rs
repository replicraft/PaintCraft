//! Cooperative cancellation at codec event, task and input boundaries.
use super::*;
use ::jxl::api::{JxlBitstreamInput, JxlParallelRunner, JxlParallelRunnerFun};
use std::{cell::RefCell, io::IoSliceMut};

pub(super) struct Execution<'a> {
    pub check: &'a dyn Fn() -> Result<()>,
    failure: RefCell<Option<Error>>,
}
impl<'a> Execution<'a> {
    pub fn new(check: &'a dyn Fn() -> Result<()>) -> Self {
        Self {
            check,
            failure: RefCell::new(None),
        }
    }
    fn checkpoint(&self) -> std::io::Result<()> {
        if let Err(error) = (self.check)() {
            *self.failure.borrow_mut() = Some(error);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "JPEG XL cancelled",
            ));
        }
        Ok(())
    }
    pub fn map(&self, error: ::jxl::error::Error) -> Error {
        if let Some(error) = self.failure.borrow_mut().take() {
            return error;
        }
        match error {
            ::jxl::error::Error::OutOfMemory(_)
            | ::jxl::error::Error::ImageOutOfMemory(..)
            | ::jxl::error::Error::ImageSizeTooLarge(..)
            | ::jxl::error::Error::ImageDimensionTooLarge(..)
            | ::jxl::error::Error::IccTooLarge => {
                resource("JPEG XL decoder resource limit exceeded")
            }
            error => codec(format!("JPEG XL decode failed: {error}")),
        }
    }
}
pub(super) struct Input<'a, 'b> {
    pub bytes: &'a [u8],
    pub execution: &'b Execution<'b>,
}
impl JxlBitstreamInput for Input<'_, '_> {
    fn available_bytes(&mut self) -> std::io::Result<usize> {
        self.execution.checkpoint()?;
        Ok(self.bytes.len())
    }
    fn read(&mut self, bufs: &mut [IoSliceMut]) -> std::io::Result<usize> {
        self.execution.checkpoint()?;
        std::io::Read::read_vectored(&mut self.bytes, bufs)
    }
    fn skip(&mut self, bytes: usize) -> std::io::Result<usize> {
        self.execution.checkpoint()?;
        let skipped = bytes.min(self.bytes.len());
        self.bytes = &self.bytes[skipped..];
        Ok(skipped)
    }
}
pub(super) struct Runner<'a>(pub &'a Execution<'a>);
impl JxlParallelRunner for Runner<'_> {
    fn run(&mut self, num: usize, fun: &JxlParallelRunnerFun<'_>) -> ::jxl::error::Result<()> {
        for i in 0..num {
            self.0
                .checkpoint()
                .map_err(|_| ::jxl::error::Error::ApiUsageError("JPEG XL cancelled"))?;
            fun(i)?;
        }
        Ok(())
    }
    fn num_threads(&self) -> usize {
        1
    }
    fn run_ordered(
        &mut self,
        num: usize,
        _max_threads: Option<usize>,
        fun: &JxlParallelRunnerFun<'_>,
    ) -> ::jxl::error::Result<()> {
        self.run(num, fun)
    }
}
