"""Bounded authored-fixture oracle; libheif1.20.2 ABI from its pinned public headers.
Only used on the owned32x24 fixtures, never production/untrusted decode.
"""
import ctypes as c
from pathlib import Path

import pillow_heif


class Error(c.Structure):
    _fields_ = [("code", c.c_int), ("subcode", c.c_int), ("message", c.c_char_p)]


class Conversion(c.Structure):
    _fields_ = [
        ("version", c.c_uint8),
        ("downsampling", c.c_int),
        ("upsampling", c.c_int),
        ("only_preferred", c.c_uint8),
    ]


class OptionsPrefix(c.Structure):
    # Public heif_decoding_options through version5. Allocated by libheif,
    # never allocated at this prefix size. Later version fields stay unchanged.
    _fields_ = [
        ("version", c.c_uint8),
        ("ignore_transformations", c.c_uint8),
        ("start_progress", c.c_void_p),
        ("on_progress", c.c_void_p),
        ("end_progress", c.c_void_p),
        ("progress_user_data", c.c_void_p),
        ("convert_hdr_to_8bit", c.c_uint8),
        ("strict_decoding", c.c_uint8),
        ("decoder_id", c.c_char_p),
        ("conversion", Conversion),
    ]


def decode(path, alpha, upsampling, native=False, width=32, height=24):
    library = Path(pillow_heif.__file__).parent / ".dylibs/libheif.1.20.2.dylib"
    lib = c.CDLL(str(library))

    def function(name, result, arguments):
        value = getattr(lib, name)
        value.restype, value.argtypes = result, arguments
        return value

    pointer = c.c_void_p
    alloc = function("heif_context_alloc", pointer, [])
    read = function("heif_context_read_from_file", Error, [pointer, c.c_char_p, pointer])
    primary = function("heif_context_get_primary_image_handle", Error, [pointer, c.POINTER(pointer)])
    options_alloc = function("heif_decoding_options_alloc", pointer, [])
    options_free = function("heif_decoding_options_free", None, [pointer])
    decode_image = function("heif_decode_image", Error, [pointer, c.POINTER(pointer), c.c_int, c.c_int, pointer])
    plane = function("heif_image_get_plane_readonly", pointer, [pointer, c.c_int, c.POINTER(c.c_int)])
    image_free = function("heif_image_release", None, [pointer])
    handle_free = function("heif_image_handle_release", None, [pointer])
    context_free = function("heif_context_free", None, [pointer])

    def checked(error):
        if error.code:
            raise RuntimeError((error.code, error.subcode, error.message))

    context, handle, image, options = alloc(), pointer(), pointer(), options_alloc()
    try:
        prefix = c.cast(options, c.POINTER(OptionsPrefix)).contents
        if prefix.version < 5 or prefix.conversion.version != 1:
            raise RuntimeError("unexpected pinned libheif option ABI")
        prefix.ignore_transformations = 1
        prefix.convert_hdr_to_8bit = 0
        prefix.strict_decoding = 1
        prefix.conversion.upsampling = upsampling
        prefix.conversion.only_preferred = 1
        checked(read(context, str(path).encode(), None))
        checked(primary(context, c.byref(handle)))
        # Public enums: RGB1; interleavedRGB10/RGBA11; channel_interleaved10.
        checked(decode_image(handle, c.byref(image), 0 if native else 1, 1 if native else (11 if alpha else 10), options))
        if native:
            output = []
            for channel in (0, 1, 2):
                stride = c.c_int()
                data = plane(image, channel, c.byref(stride))
                pw, ph = (width, height) if channel == 0 else (width // 2, height // 2)
                if not data or stride.value < pw:
                    raise RuntimeError("invalid oracle native plane")
                output.append(b"".join(c.string_at(data + y * stride.value, pw) for y in range(ph)))
            return b"".join(output)
        stride = c.c_int()
        data = plane(image, 10, c.byref(stride))
        row_bytes = width * (4 if alpha else 3)
        if not data or stride.value < row_bytes:
            raise RuntimeError("invalid oracle interleaved plane")
        return b"".join(c.string_at(data + y * stride.value, row_bytes) for y in range(height))
    finally:
        if image:
            image_free(image)
        if handle:
            handle_free(handle)
        options_free(options)
        context_free(context)
