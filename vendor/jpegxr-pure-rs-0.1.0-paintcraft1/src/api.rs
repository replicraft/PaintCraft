use crate::common::include::guiddef::GUID;
use crate::image::sys::strcodec::{create_ws_file_owned, WsFileMode};
use crate::image::sys::windowsmediaphoto::{tagCWMIStrCodecParam, tagCWMTranscodingParam};
use crate::jxrgluelib::jxrglue::{
    pixel_format_lookup, pk_codec_factory_create_decoder_from_file, tagPKImageDecode,
    tagPKPixelInfo, tagPKRect, BitDepth as JxrBitDepth, ColorFormat as JxrColorFormat,
    GUID_PKPixelFormat24bppRGB, GUID_PKPixelFormatDontCare, PixelFormatFlags,
    PixelFormatInterpretation as JxrInterpretation, LOOKUP_FORWARD,
};
use crate::jxrgluelib::jxrglue_jxr::pk_image_encode_create_wmp_owned;
use crate::WmpError;
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Codec(WmpError),
    Io(std::io::Error),
    /// The caller-provided maximum output extent was exceeded.
    OutputLimit,
    InvalidPixelFormat,
    InvalidDimensions,
    InvalidStride,
    BufferTooSmall,
    SizeOverflow,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Codec(err) => write!(f, "JPEG-XR codec error: {err}"),
            Error::Io(err) => write!(f, "I/O error: {err}"),
            Error::OutputLimit => f.write_str("JPEG-XR output extent limit"),
            Error::InvalidPixelFormat => f.write_str("invalid JPEG-XR pixel format"),
            Error::InvalidDimensions => f.write_str("invalid image dimensions"),
            Error::InvalidStride => f.write_str("invalid image stride"),
            Error::BufferTooSmall => f.write_str("pixel buffer is too small"),
            Error::SizeOverflow => f.write_str("image size overflows addressable memory"),
        }
    }
}

impl std::error::Error for Error {}

impl From<WmpError> for Error {
    fn from(value: WmpError) -> Self {
        Error::Codec(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::Io(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelFormat {
    guid: GUID,
    bits_per_pixel: usize,
    channel_count: u8,
    bits_per_sample: u8,
    sample_count: u8,
    sample_format: SampleFormat,
    bit_depth: BitDepth,
    color_format: ColorFormat,
    interpretation: PixelInterpretation,
    channel_order: ChannelOrder,
    has_alpha: bool,
    premultiplied_alpha: bool,
}

impl PixelFormat {
    /// Construct fresh unsigned RGB8 pixels using the existing checked lookup.
    pub fn rgb8() -> Result<Self> {
        Self::from_guid(GUID_PKPixelFormat24bppRGB)
    }

    pub fn bits_per_pixel(self) -> usize {
        self.bits_per_pixel
    }

    pub fn bytes_per_pixel(self) -> usize {
        self.bits_per_pixel.div_ceil(8)
    }

    pub fn minimum_stride(self, width: u32) -> Result<usize> {
        checked_stride(width, self)
    }

    pub fn channel_count(self) -> u8 {
        self.channel_count
    }

    pub fn bits_per_sample(self) -> u8 {
        self.bits_per_sample
    }

    pub fn sample_count(self) -> u8 {
        self.sample_count
    }

    pub fn sample_format(self) -> SampleFormat {
        self.sample_format
    }

    pub fn bit_depth(self) -> BitDepth {
        self.bit_depth
    }

    pub fn color_format(self) -> ColorFormat {
        self.color_format
    }

    pub fn interpretation(self) -> PixelInterpretation {
        self.interpretation
    }

    pub fn channel_order(self) -> ChannelOrder {
        self.channel_order
    }

    pub fn has_alpha(self) -> bool {
        self.has_alpha
    }

    pub fn premultiplied_alpha(self) -> bool {
        self.premultiplied_alpha
    }

    pub fn is_bgr_order(self) -> bool {
        self.channel_order == ChannelOrder::Bgr
    }

    fn from_guid(guid: GUID) -> Result<Self> {
        let mut info = tagPKPixelInfo {
            pGUIDPixFmt: Some(guid),
            cChannel: 0,
            cfColorFormat: JxrColorFormat::YOnly,
            bdBitDepth: JxrBitDepth::One,
            cbitUnit: 0,
            grBit: PixelFormatFlags::NONE,
            uInterpretation: JxrInterpretation::WhiteIsZero,
            uSamplePerPixel: 0,
            uBitsPerSample: 0,
            uSampleFormat: 0,
        };
        unsafe {
            pixel_format_lookup(&mut info, LOOKUP_FORWARD)?;
        }
        if info.pGUIDPixFmt == Some(GUID_PKPixelFormatDontCare) || info.cbitUnit == 0 {
            return Err(Error::InvalidPixelFormat);
        }
        Ok(Self {
            guid,
            bits_per_pixel: info.cbitUnit as usize,
            channel_count: info
                .cChannel
                .try_into()
                .map_err(|_| Error::InvalidPixelFormat)?,
            bits_per_sample: info
                .uBitsPerSample
                .try_into()
                .map_err(|_| Error::InvalidPixelFormat)?,
            sample_count: info
                .uSamplePerPixel
                .try_into()
                .map_err(|_| Error::InvalidPixelFormat)?,
            sample_format: SampleFormat::from_jxr(info.uSampleFormat),
            bit_depth: BitDepth::from_jxr(info.bdBitDepth),
            color_format: ColorFormat::from_jxr(info.cfColorFormat),
            interpretation: PixelInterpretation::from_jxr(info.uInterpretation),
            channel_order: if (info.grBit & PixelFormatFlags::BGR) == PixelFormatFlags::BGR {
                ChannelOrder::Bgr
            } else if info.uInterpretation == JxrInterpretation::Rgb {
                ChannelOrder::Rgb
            } else {
                ChannelOrder::Unspecified
            },
            has_alpha: (info.grBit & PixelFormatFlags::HAS_ALPHA) == PixelFormatFlags::HAS_ALPHA,
            premultiplied_alpha: (info.grBit & PixelFormatFlags::PRE_MULTIPLIED)
                == PixelFormatFlags::PRE_MULTIPLIED,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelOrder {
    Unspecified,
    Rgb,
    Bgr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorFormat {
    Luma,
    Yuv420,
    Yuv422,
    Yuv444,
    Cmyk,
    NComponent,
    Rgb,
    Rgbe,
    Other,
}

impl ColorFormat {
    fn from_jxr(value: JxrColorFormat) -> Self {
        match value {
            JxrColorFormat::YOnly => Self::Luma,
            JxrColorFormat::Yuv420 => Self::Yuv420,
            JxrColorFormat::Yuv422 => Self::Yuv422,
            JxrColorFormat::Yuv444 => Self::Yuv444,
            JxrColorFormat::Cmyk => Self::Cmyk,
            JxrColorFormat::NComponent => Self::NComponent,
            JxrColorFormat::Rgb => Self::Rgb,
            JxrColorFormat::Rgbe => Self::Rgbe,
            JxrColorFormat::Max => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitDepth {
    One,
    Eight,
    Sixteen,
    SignedSixteen,
    HalfFloat,
    ThirtyTwo,
    SignedThirtyTwo,
    Float,
    Five,
    Ten,
    FiveSixFive,
    Other,
}

impl BitDepth {
    fn from_jxr(value: JxrBitDepth) -> Self {
        match value {
            JxrBitDepth::One | JxrBitDepth::OneAlt => Self::One,
            JxrBitDepth::Eight => Self::Eight,
            JxrBitDepth::Sixteen => Self::Sixteen,
            JxrBitDepth::SixteenS => Self::SignedSixteen,
            JxrBitDepth::SixteenF => Self::HalfFloat,
            JxrBitDepth::ThirtyTwo => Self::ThirtyTwo,
            JxrBitDepth::ThirtyTwoS => Self::SignedThirtyTwo,
            JxrBitDepth::ThirtyTwoF => Self::Float,
            JxrBitDepth::Five => Self::Five,
            JxrBitDepth::Ten => Self::Ten,
            JxrBitDepth::FiveSixFive => Self::FiveSixFive,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelInterpretation {
    WhiteIsZero,
    BlackIsZero,
    Rgb,
    Cmyk,
    NChannel,
    Rgbe,
    Other,
}

impl PixelInterpretation {
    fn from_jxr(value: JxrInterpretation) -> Self {
        match value {
            JxrInterpretation::WhiteIsZero => Self::WhiteIsZero,
            JxrInterpretation::BlackIsZero => Self::BlackIsZero,
            JxrInterpretation::Rgb => Self::Rgb,
            JxrInterpretation::Cmyk => Self::Cmyk,
            JxrInterpretation::NChannel => Self::NChannel,
            JxrInterpretation::Rgbe => Self::Rgbe,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    Unspecified,
    UnsignedInteger,
    FixedPoint,
    FloatingPoint,
    Other(u32),
}

impl SampleFormat {
    fn from_jxr(value: u32) -> Self {
        match value {
            0 => Self::Unspecified,
            1 => Self::UnsignedInteger,
            2 => Self::FixedPoint,
            3 => Self::FloatingPoint,
            other => Self::Other(other),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecodedImage {
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
    stride: usize,
    pixels: Vec<u8>,
}

impl DecodedImage {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    pub fn stride(&self) -> usize {
        self.stride
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }
}

pub struct Decoder {
    decoder: Box<tagPKImageDecode>,
    _input: DecoderInput,
}

enum DecoderInput {
    File,
    TempFile(PathBuf),
}

impl Decoder {
    /// Creates a decoder from an already-loaded JPEG-XR buffer.
    ///
    /// The API borrows the caller's slice instead of taking ownership or cloning
    /// it. The current backend may still create a temporary seekable backing
    /// store internally because the translated JXRLib stream layer requires
    /// seekable input.
    pub fn from_slice(data: &[u8]) -> Result<Self> {
        let path = temp_jxr_path("input");
        std::fs::write(&path, data)?;
        let decoder = create_decoder_from_path(&path)?;
        Ok(Self {
            decoder,
            _input: DecoderInput::TempFile(path),
        })
    }

    pub fn dimensions(&mut self) -> Result<(u32, u32)> {
        let mut width = 0;
        let mut height = 0;
        unsafe {
            let get_size = self.decoder.GetSize.ok_or(WmpError::AbstractMethod)?;
            get_size(&mut *self.decoder, Some(&mut width), Some(&mut height))?;
        }
        if width <= 0 || height <= 0 {
            return Err(Error::InvalidDimensions);
        }
        Ok((width as u32, height as u32))
    }

    pub fn pixel_format(&mut self) -> Result<PixelFormat> {
        let mut guid = GUID_PKPixelFormatDontCare;
        unsafe {
            let get_pixel_format = self
                .decoder
                .GetPixelFormat
                .ok_or(WmpError::AbstractMethod)?;
            get_pixel_format(&mut *self.decoder, Some(&mut guid))?;
        }
        PixelFormat::from_guid(guid)
    }

    pub fn decode(&mut self) -> Result<DecodedImage> {
        let (width, height) = self.dimensions()?;
        let pixel_format = self.pixel_format()?;
        let stride = checked_stride(width, pixel_format)?;
        let len = checked_len(stride, height)?;
        let mut pixels = vec![0; len];
        self.decode_into(&mut pixels, stride)?;
        Ok(DecodedImage {
            width,
            height,
            pixel_format,
            stride,
            pixels,
        })
    }

    pub fn decode_into(&mut self, pixels: &mut [u8], stride: usize) -> Result<()> {
        let (width, height) = self.dimensions()?;
        let pixel_format = self.pixel_format()?;
        validate_pixel_buffer(width, height, pixel_format, pixels.len(), stride)?;
        let rect = tagPKRect {
            X: 0,
            Y: 0,
            Width: width as i32,
            Height: height as i32,
        };
        unsafe {
            let copy = self.decoder.Copy.ok_or(WmpError::AbstractMethod)?;
            copy(&mut *self.decoder, Some(&rect), pixels, stride as u32)?;
        }
        Ok(())
    }
    /// Opens a file-backed decoder.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let decoder = create_decoder_from_path(path.as_ref())?;
        Ok(Self {
            decoder,
            _input: DecoderInput::File,
        })
    }

    /// Reads all bytes from `reader` into an owned in-memory buffer before decoding.
    pub fn from_reader(mut reader: impl Read) -> Result<Self> {
        let mut input = Vec::new();
        reader.read_to_end(&mut input)?;
        Self::from_bytes(input)
    }

    /// Creates a decoder that owns an already-loaded JPEG-XR buffer without
    /// cloning it at the API boundary. The current backend may still create a
    /// temporary seekable backing store internally.
    pub fn from_bytes(input: Vec<u8>) -> Result<Self> {
        let path = temp_jxr_path("input");
        std::fs::write(&path, &input)?;
        let decoder = create_decoder_from_path(&path)?;
        Ok(Self {
            decoder,
            _input: DecoderInput::TempFile(path),
        })
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        if let DecoderInput::TempFile(path) = &self._input {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub struct Encoder {
    options: EncoderOptions,
}

#[derive(Debug, Clone)]
pub struct EncoderOptions {
    pub color_format: EncoderColorFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderColorFormat {
    Yuv444,
}

impl Default for EncoderOptions {
    fn default() -> Self {
        Self {
            color_format: EncoderColorFormat::Yuv444,
        }
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Encoder {
    pub fn new() -> Self {
        Self {
            options: EncoderOptions::default(),
        }
    }

    pub fn with_options(options: EncoderOptions) -> Self {
        Self { options }
    }

    pub fn encode_image_to_path(&self, image: &DecodedImage, path: impl AsRef<Path>) -> Result<()> {
        self.encode_pixels_to_path(
            image.width,
            image.height,
            image.pixel_format,
            image.pixels(),
            image.stride,
            path,
        )
    }

    pub fn encode_image_to_writer(&self, image: &DecodedImage, writer: impl Write) -> Result<()> {
        self.encode_pixels_to_writer(
            image.width,
            image.height,
            image.pixel_format,
            image.pixels(),
            image.stride,
            writer,
        )
    }

    pub fn encode_image_to_vec(&self, image: &DecodedImage) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        self.encode_image_to_writer(image, &mut output)?;
        Ok(output)
    }

    pub fn encode_pixels_to_writer(
        &self,
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        pixels: &[u8],
        stride: usize,
        mut writer: impl Write,
    ) -> Result<()> {
        let path = temp_jxr_path("encode");
        let result = (|| {
            self.encode_pixels_to_path(width, height, pixel_format, pixels, stride, &path)?;
            let mut file = File::open(&path)?;
            std::io::copy(&mut file, &mut writer)?;
            Ok(())
        })();
        let _ = std::fs::remove_file(&path);
        result
    }

    /// Encode to an empty, position-zero caller-owned sink without opening paths.
    /// Every stream operation checks cancellation and the maximum seek/write extent.
    /// On failure the sink may contain a bounded partial result; the caller must
    /// discard it. Cancellation is checked at stream boundaries, not CPU preemption.
    /// OutputLimit is distinct from sink/check IO errors, including ErrorKind::Other.
    pub fn encode_pixels_to_seekable<W: Write + Seek, C: FnMut() -> std::io::Result<()>>(
        &self,
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        pixels: &[u8],
        stride: usize,
        sink: &mut W,
        max_extent: u64,
        check: &mut C,
    ) -> Result<()> {
        validate_pixel_buffer(width, height, pixel_format, pixels.len(), stride)?;
        // The closure drops its encoder before the borrowed stream/context ends.
        let result = unsafe {
            crate::image::sys::strcodec::with_ws_output(sink, max_extent, check, |stream| {
                let mut encoder = pk_image_encode_create_wmp_owned();
                let params = tagCWMIStrCodecParam {
                    cfColorFormat: match self.options.color_format {
                        EncoderColorFormat::Yuv444 => JxrColorFormat::Yuv444,
                    },
                    ..tagCWMIStrCodecParam::default()
                };
                let initialize = encoder.Initialize.ok_or(WmpError::AbstractMethod)?;
                initialize(
                    &mut *encoder,
                    stream,
                    Some(&params),
                    std::mem::size_of::<tagCWMIStrCodecParam>(),
                )?;
                let set_pixel_format = encoder.SetPixelFormat.ok_or(WmpError::AbstractMethod)?;
                set_pixel_format(&mut *encoder, pixel_format.guid)?;
                let set_size = encoder.SetSize.ok_or(WmpError::AbstractMethod)?;
                set_size(&mut *encoder, width as i32, height as i32)?;
                let write_pixels = encoder.WritePixels.ok_or(WmpError::AbstractMethod)?;
                write_pixels(&mut *encoder, height, pixels, stride as u32)?;
                drop(encoder);
                Ok(())
            })
        }
        .map_err(|fault| match fault {
            crate::image::sys::strcodec::OutputFault::Limit => Error::OutputLimit,
            crate::image::sys::strcodec::OutputFault::Io(error) => Error::Io(error),
        })?;
        result.map_err(Error::Codec)
    }

    pub fn encode_pixels_to_path(
        &self,
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        pixels: &[u8],
        stride: usize,
        path: impl AsRef<Path>,
    ) -> Result<()> {
        validate_pixel_buffer(width, height, pixel_format, pixels.len(), stride)?;
        unsafe {
            let mut stream = create_ws_file_owned(path.as_ref(), WsFileMode::ReadWriteTruncate)?;
            let mut encoder = pk_image_encode_create_wmp_owned();
            let params = tagCWMIStrCodecParam {
                cfColorFormat: match self.options.color_format {
                    EncoderColorFormat::Yuv444 => JxrColorFormat::Yuv444,
                },
                ..tagCWMIStrCodecParam::default()
            };
            let initialize = encoder.Initialize.ok_or(WmpError::AbstractMethod)?;
            initialize(
                &mut *encoder,
                &mut *stream,
                Some(&params),
                std::mem::size_of::<tagCWMIStrCodecParam>(),
            )?;
            let set_pixel_format = encoder.SetPixelFormat.ok_or(WmpError::AbstractMethod)?;
            set_pixel_format(&mut *encoder, pixel_format.guid)?;
            let set_size = encoder.SetSize.ok_or(WmpError::AbstractMethod)?;
            set_size(&mut *encoder, width as i32, height as i32)?;
            let write_pixels = encoder.WritePixels.ok_or(WmpError::AbstractMethod)?;
            write_pixels(&mut *encoder, height, pixels, stride as u32)?;
            drop(encoder);
            drop(stream);
        }
        Ok(())
    }
}

pub fn decode_file(path: impl AsRef<Path>) -> Result<DecodedImage> {
    Decoder::open(path)?.decode()
}

pub fn decode_reader(reader: impl Read) -> Result<DecodedImage> {
    Decoder::from_reader(reader)?.decode()
}

/// Decodes an already-loaded JPEG-XR buffer without copying the input bytes.
pub fn decode_bytes(data: &[u8]) -> Result<DecodedImage> {
    Decoder::from_slice(data)?.decode()
}

pub fn encode_to_writer(image: &DecodedImage, writer: impl Write) -> Result<()> {
    Encoder::new().encode_image_to_writer(image, writer)
}

pub fn encode_to_vec(image: &DecodedImage) -> Result<Vec<u8>> {
    Encoder::new().encode_image_to_vec(image)
}

pub fn encode_to_path(image: &DecodedImage, path: impl AsRef<Path>) -> Result<()> {
    Encoder::new().encode_image_to_path(image, path)
}

pub fn transcode_file_to_path(input: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<()> {
    let mut bytes = Vec::new();
    File::open(input)?.read_to_end(&mut bytes)?;
    transcode_bytes_to_path(bytes, output)
}

pub fn transcode_reader_to_writer(reader: impl Read, writer: impl Write) -> Result<()> {
    let mut decoder = Decoder::from_reader(reader)?;
    transcode_decoder_to_writer(&mut decoder, writer)
}

pub fn transcode_bytes_to_vec(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = Decoder::from_slice(data)?;
    let mut output = Vec::new();
    transcode_decoder_to_writer(&mut decoder, &mut output)?;
    Ok(output)
}

pub fn transcode_bytes_to_path(input: Vec<u8>, output: impl AsRef<Path>) -> Result<()> {
    let mut decoder = Decoder::from_bytes(input)?;
    transcode_decoder_to_path(&mut decoder, output)
}

fn transcode_decoder_to_writer(decoder: &mut Decoder, mut writer: impl Write) -> Result<()> {
    let path = temp_jxr_path("transcode");
    let result = (|| {
        transcode_decoder_to_path(decoder, &path)?;
        let mut file = File::open(&path)?;
        std::io::copy(&mut file, &mut writer)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&path);
    result
}

fn transcode_decoder_to_path(decoder: &mut Decoder, output: impl AsRef<Path>) -> Result<()> {
    let (width, height) = decoder.dimensions()?;
    unsafe {
        let mut stream = create_ws_file_owned(output.as_ref(), WsFileMode::ReadWriteTruncate)?;
        let mut encoder = pk_image_encode_create_wmp_owned();
        let params = tagCWMIStrCodecParam::default();
        let initialize = encoder.Initialize.ok_or(WmpError::AbstractMethod)?;
        initialize(
            &mut *encoder,
            &mut *stream,
            Some(&params),
            std::mem::size_of::<tagCWMIStrCodecParam>(),
        )?;

        let mut transcode = tagCWMTranscodingParam {
            cLeftX: 0,
            cWidth: width as usize,
            cTopY: 0,
            cHeight: height as usize,
            bfBitstreamFormat: decoder.decoder.WMP.wmiSCP.bfBitstreamFormat,
            uAlphaMode: decoder.decoder.WMP.wmiSCP.uAlphaMode,
            sbSubband: decoder.decoder.WMP.wmiSCP.sbSubband,
            oOrientation: decoder.decoder.WMP.wmiI.oOrientation,
            bIgnoreOverlap: decoder.decoder.WMP.bIgnoreOverlap,
        };
        let transcode_fn = encoder.Transcode.ok_or(WmpError::AbstractMethod)?;
        transcode_fn(&mut *encoder, &mut *decoder.decoder, &mut transcode)?;
        drop(encoder);
        drop(stream);
    }
    Ok(())
}

fn create_decoder_from_path(path: &Path) -> Result<Box<tagPKImageDecode>> {
    unsafe { pk_codec_factory_create_decoder_from_file(Some(path)).map_err(Error::from) }
}

fn checked_stride(width: u32, pixel_format: PixelFormat) -> Result<usize> {
    (width as usize)
        .checked_mul(pixel_format.bits_per_pixel())
        .and_then(|bits| bits.checked_add(7))
        .map(|bits| bits / 8)
        .ok_or(Error::SizeOverflow)
}

fn checked_len(stride: usize, height: u32) -> Result<usize> {
    stride
        .checked_mul(height as usize)
        .ok_or(Error::SizeOverflow)
}

fn validate_pixel_buffer(
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
    buffer_len: usize,
    stride: usize,
) -> Result<()> {
    // SetSize accepts signed i32 dimensions. Reject before any output access.
    if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
        return Err(Error::InvalidDimensions);
    }
    let min_stride = checked_stride(width, pixel_format)?;
    if stride < min_stride || stride > u32::MAX as usize {
        return Err(Error::InvalidStride);
    }
    let min_len = checked_len(stride, height)?;
    if buffer_len < min_len {
        return Err(Error::BufferTooSmall);
    }
    Ok(())
}

fn temp_jxr_path(label: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "jpegxr-pure-rs-{label}-{}-{}.jxr",
        std::process::id(),
        next_temp_id()
    ));
    path
}

fn next_temp_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
