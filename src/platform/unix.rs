// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0

#![cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd"
))]

use std::sync::Arc;

pub use crate::platform_impl::{ hit_test, x11 };
use crate::window::{Window, WindowBuilder};
use crate::event_loop::EventLoopWindowTarget;

/// Additional methods on `EventLoopWindowTarget` that are specific to Unix.
pub trait EventLoopWindowTargetExtUnix {
    fn xlib_xconnection(&self) -> Option<Arc<x11::XConnection>>;
}

impl<T> EventLoopWindowTargetExtUnix for EventLoopWindowTarget<T> {
    fn xlib_xconnection(&self) -> Option<Arc<x11::XConnection>> {
        let xlib = x11::X11_BACKEND.lock();
        if let Ok(x) = &*xlib {
            Some(x.clone())
        } else {
            None
        }
    }
}

/// Additional methods on `Window` that are specific to Unix.
pub trait WindowExtUnix {
  /// Returns the `ApplicatonWindow` from gtk crate that is used by this window.
  fn gtk_window(&self) -> &gtk::ApplicationWindow;

  /// Whether to show the window icon in the taskbar or not.
  fn set_skip_taskbar(&self, skip: bool);
}

impl WindowExtUnix for Window {
  fn gtk_window(&self) -> &gtk::ApplicationWindow {
    &self.window.window
  }

  fn set_skip_taskbar(&self, skip: bool) {
    self.window.set_skip_taskbar(skip);
  }
}

/// Additional methods on `WindowBuilder` that are specific to Unix.
pub trait WindowBuilderExtUnix {
  /// Whether to create the window icon with the taskbar icon or not.
  fn with_skip_taskbar(self, skip: bool) -> WindowBuilder;
}

impl WindowBuilderExtUnix for WindowBuilder {
  fn with_skip_taskbar(mut self, skip: bool) -> WindowBuilder {
    self.platform_specific.skip_taskbar = skip;
    self
  }
}
