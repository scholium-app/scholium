"""Private X11 front-buffer reader for native LaTeX acceptance."""
import ctypes
import hashlib

class XImage(ctypes.Structure):
    _fields_ = [("width", ctypes.c_int), ("height", ctypes.c_int), ("xoffset", ctypes.c_int),
                ("format", ctypes.c_int), ("data", ctypes.c_void_p), ("byte_order", ctypes.c_int),
                ("bitmap_unit", ctypes.c_int), ("bitmap_bit_order", ctypes.c_int),
                ("bitmap_pad", ctypes.c_int), ("depth", ctypes.c_int),
                ("bytes_per_line", ctypes.c_int), ("bits_per_pixel", ctypes.c_int)]


class Screen:
    """XGetImage reads the actual private Xvfb front buffer, after GPU presentation."""
    def __init__(self, window):
        self.x = ctypes.CDLL("libX11.so.6")
        self.x.XOpenDisplay.argtypes = [ctypes.c_char_p]
        self.x.XOpenDisplay.restype = ctypes.c_void_p
        self.x.XGetImage.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_int,
                                    ctypes.c_uint, ctypes.c_uint, ctypes.c_ulong, ctypes.c_int]
        self.x.XGetImage.restype = ctypes.POINTER(XImage)
        self.x.XDestroyImage.argtypes = [ctypes.POINTER(XImage)]
        self.x.XCloseDisplay.argtypes = [ctypes.c_void_p]
        self.display = self.x.XOpenDisplay(None)
        assert self.display
        self.window = int(window)

    def pixels(self, rect):
        x, y, right, bottom = map(round, rect)
        image = self.x.XGetImage(self.display, self.window, x, y, right-x, bottom-y, 0xffffffff, 2)
        assert image
        data = ctypes.string_at(image.contents.data, image.contents.bytes_per_line * image.contents.height)
        self.x.XDestroyImage(image)
        return hashlib.sha256(data).hexdigest()

    def close(self):
        self.x.XCloseDisplay(self.display)
