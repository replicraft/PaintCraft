//! Borrowed launch context and parent-owned seekable codec backing.
use super::*;
use paintcraft_platform::source::PrivateSource;
use std::ffi::OsString;
/// The service retains all argument/backing ownership across this callback.
/// The injected supervisor must terminate/reap/join before returning or unwinding.
#[derive(Debug)]
pub struct Launch<'a> {
    pub executable: &'a Path,
    pub request: &'a Request,
    pub arguments: &'a [OsString],
    pub remaining: Duration,
}
pub(super) struct Backing {
    source: Option<PrivateSource>,
    arguments: Vec<OsString>,
}
impl Backing {
    pub fn new(request: &Request, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        if request.header.codec != Codec::JxrRgb8 {
            return Ok(Self {
                source: None,
                arguments: Vec::new(),
            });
        }
        crate::jxr::inspect_checked(
            request.source.as_slice(),
            u64::from(request.header.pixels),
            check,
        )?;
        let source = PrivateSource::create(
            request.source.as_slice(),
            "jxr",
            INPUT_BYTES_MAX as u64,
            check,
        )?;
        source.identity().verify_bytes(
            source.path(),
            request.header.length,
            &request.header.sha256,
            check,
        )?;
        let arguments = vec![
            OsString::from("--jxr-source"),
            source.path().as_os_str().to_owned(),
        ];
        Ok(Self {
            source: Some(source),
            arguments,
        })
    }
    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }
    pub fn invoke(
        self,
        executable: &Path,
        request: &Request,
        remaining: Duration,
        check: &dyn Fn() -> Result<()>,
        launch: impl FnOnce(Launch<'_>, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    ) -> Result<Vec<u8>> {
        let result = launch(
            Launch {
                executable,
                request,
                arguments: self.arguments(),
                remaining,
            },
            check,
        );
        let cleanup = self.close();
        let output = result?;
        cleanup?;
        Ok(output)
    }
    /// Invoke only after the supervised callback returned/reaped. Errors before
    /// launch and callback unwinds still retain the ordinary RAII cleanup path.
    pub fn close(self) -> Result<()> {
        self.source.map_or(Ok(()), PrivateSource::close)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backing_exists_only_for_jxr_and_preserves_exact_arguments_and_source() {
        let owner = Owner {
            session: Uuid::new_v4(),
            state: Uuid::new_v4(),
            epoch: 0,
            frontend: 3,
        };
        let bytes = include_bytes!("../../../tests/jxr-fixtures/rgb8-4x3.jxr");
        let request =
            Request::capture_for(Codec::JxrRgb8, Arc::new(bytes.to_vec()), owner, PIXELS_MAX)
                .unwrap();
        let backing = Backing::new(&request, &|| Ok(())).unwrap();
        assert_eq!(backing.arguments()[0], "--jxr-source");
        let path = PathBuf::from(&backing.arguments()[1]);
        let directory = path.parent().unwrap().to_path_buf();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        backing.close().unwrap();
        assert!(!directory.exists());
        let other = Request::capture_for(Codec::JxlRgba8, Arc::new(vec![1]), owner, 1).unwrap();
        let backing = Backing::new(&other, &|| Ok(())).unwrap();
        assert!(backing.arguments().is_empty());
        backing.close().unwrap();
    }
    #[test]
    fn callback_refusal_and_unwind_cleanup_private_source_after_callback_scope() {
        use std::cell::RefCell;
        let request = Request::capture_for(
            Codec::JxrRgb8,
            Arc::new(include_bytes!("../../../tests/jxr-fixtures/rgb8-4x3.jxr").to_vec()),
            Owner {
                session: Uuid::new_v4(),
                state: Uuid::new_v4(),
                epoch: 0,
                frontend: 3,
            },
            PIXELS_MAX,
        )
        .unwrap();
        for unwind in [false, true] {
            let observed = RefCell::new(None);
            let backing = Backing::new(&request, &|| Ok(())).unwrap();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                backing.invoke(
                    Path::new("/unused-test-helper"),
                    &request,
                    Duration::from_secs(1),
                    &|| Ok(()),
                    |launch, _| {
                        let path = PathBuf::from(&launch.arguments[1]);
                        assert_eq!(std::fs::read(&path).unwrap(), request.source.as_slice());
                        *observed.borrow_mut() = Some(path.parent().unwrap().to_path_buf());
                        if unwind {
                            panic!("callback unwind after owned supervisor scope");
                        }
                        Err(Error::new(
                            ErrorCode::OperationCancelled,
                            "callback refuses after supervised cleanup",
                        ))
                    },
                )
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert_eq!(
                    result.unwrap().unwrap_err().code,
                    ErrorCode::OperationCancelled
                );
            }
            assert!(!observed.into_inner().unwrap().exists());
        }
    }
}
