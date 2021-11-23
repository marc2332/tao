use std::{error::Error, fmt, os::raw::c_int, ptr, sync::Arc, ffi::CStr, mem::MaybeUninit, os::raw::*};

use x11_dl::{ error::OpenError, xlib, xrender, xrandr, xcursor, xinput2, xlib_xcb};
use parking_lot::Mutex;

lazy_static! {
    pub static ref X11_BACKEND: Mutex<Result<Arc<XConnection>, XNotSupported>> =
        Mutex::new(XConnection::new(Some(x_error_callback)).map(Arc::new));
}

unsafe extern "C" fn x_error_callback(
    display: *mut xlib::Display,
    event: *mut xlib::XErrorEvent,
) -> c_int {
    let xconn_lock = X11_BACKEND.lock();
    if let Ok(ref xconn) = *xconn_lock {
        // `assume_init` is safe here because the array consists of `MaybeUninit` values,
        // which do not require initialization.
        let mut buf: [MaybeUninit<c_char>; 1024] = MaybeUninit::uninit().assume_init();
        (xconn.xlib.XGetErrorText)(
            display,
            (*event).error_code as c_int,
            buf.as_mut_ptr() as *mut c_char,
            buf.len() as c_int,
        );
        let description = CStr::from_ptr(buf.as_ptr() as *const c_char).to_string_lossy();

        let error = XError {
            description: description.into_owned(),
            error_code: (*event).error_code,
            request_code: (*event).request_code,
            minor_code: (*event).minor_code,
        };

        error!("X11 error: {:#?}", error);

        *xconn.latest_error.lock() = Some(error);
    }
    // Fun fact: this return value is completely ignored.
    0
}

/// A connection to an X server.
pub struct XConnection {
    pub xlib: xlib::Xlib,
    /// Exposes XRandR functions from version < 1.5
    pub xrandr: xrandr::Xrandr_2_2_0,
    /// Exposes XRandR functions from version = 1.5
    pub xrandr_1_5: Option<xrandr::Xrandr>,
    pub xcursor: xcursor::Xcursor,
    pub xinput2: xinput2::XInput2,
    pub xlib_xcb: xlib_xcb::Xlib_xcb,
    pub xrender: xrender::Xrender,
    pub display: *mut xlib::Display,
    pub x11_fd: c_int,
    pub latest_error: Mutex<Option<XError>>,
}

unsafe impl Send for XConnection {}
unsafe impl Sync for XConnection {}

pub type XErrorHandler =
    Option<unsafe extern "C" fn(*mut xlib::Display, *mut xlib::XErrorEvent) -> libc::c_int>;

impl XConnection {
    pub fn new(error_handler: XErrorHandler) -> Result<XConnection, XNotSupported> {
        // opening the libraries
        let xlib = xlib::Xlib::open()?;
        let xcursor = xcursor::Xcursor::open()?;
        let xrandr = xrandr::Xrandr_2_2_0::open()?;
        let xrandr_1_5 = xrandr::Xrandr::open().ok();
        let xinput2 = xinput2::XInput2::open()?;
        let xlib_xcb = xlib_xcb::Xlib_xcb::open()?;
        let xrender = xrender::Xrender::open()?;

        unsafe { (xlib.XInitThreads)() };
        unsafe { (xlib.XSetErrorHandler)(error_handler) };

        // calling XOpenDisplay
        let display = unsafe {
            let display = (xlib.XOpenDisplay)(ptr::null());
            if display.is_null() {
                return Err(XNotSupported::XOpenDisplayFailed);
            }
            display
        };

        // Get X11 socket file descriptor
        let x11_fd = unsafe { (xlib.XConnectionNumber)(display) };

        Ok(XConnection {
            xlib,
            xcursor,
            xinput2,
            xlib_xcb,
            xrandr,
            xrandr_1_5,
            xrender,
            display,
            x11_fd,
            latest_error: Mutex::new(None),
        })
    }

    /// Checks whether an error has been triggered by the previous function calls.
    #[inline]
    pub fn check_errors(&self) -> Result<(), XError> {
        let error = self.latest_error.lock().take();
        if let Some(error) = error {
            Err(error)
        } else {
            Ok(())
        }
    }

    /// Ignores any previous error.
    #[inline]
    pub fn ignore_error(&self) {
        *self.latest_error.lock() = None;
    }
}

impl fmt::Debug for XConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.display.fmt(f)
    }
}

impl Drop for XConnection {
    #[inline]
    fn drop(&mut self) {
        unsafe { (self.xlib.XCloseDisplay)(self.display) };
    }
}

/// Error triggered by xlib.
#[derive(Debug, Clone)]
pub struct XError {
    pub description: String,
    pub error_code: u8,
    pub request_code: u8,
    pub minor_code: u8,
}

impl Error for XError {}

impl fmt::Display for XError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            formatter,
            "X error: {} (code: {}, request code: {}, minor code: {})",
            self.description, self.error_code, self.request_code, self.minor_code
        )
    }
}

/// Error returned if this system doesn't have XLib or can't create an X connection.
#[derive(Clone, Debug)]
pub enum XNotSupported {
    /// Failed to load one or several shared libraries.
    LibraryOpenError(OpenError),
    /// Connecting to the X server with `XOpenDisplay` failed.
    XOpenDisplayFailed, // TODO: add better message
}

impl From<OpenError> for XNotSupported {
    #[inline]
    fn from(err: OpenError) -> XNotSupported {
        XNotSupported::LibraryOpenError(err)
    }
}

impl XNotSupported {
    fn description(&self) -> &'static str {
        match self {
            XNotSupported::LibraryOpenError(_) => "Failed to load one of xlib's shared libraries",
            XNotSupported::XOpenDisplayFailed => "Failed to open connection to X server",
        }
    }
}

impl Error for XNotSupported {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            XNotSupported::LibraryOpenError(ref err) => Some(err),
            _ => None,
        }
    }
}

impl fmt::Display for XNotSupported {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        formatter.write_str(self.description())
    }
}
