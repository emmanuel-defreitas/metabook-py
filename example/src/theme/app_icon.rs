//! AppKit owns the Dock icon; select the supplied artwork with Ely's mode.
#![allow(deprecated)] // Same AppKit bridge version as the pinned GPUI runtime.
use cocoa::{
    appkit::{NSApp, NSApplication, NSImage},
    base::nil,
    foundation::{NSAutoreleasePool, NSData},
};
use ely_gpui_component::theme::Mode;

/// Called only by theme synchronization on GPUI's main thread.
pub(super) fn set(mode: Mode) {
    let bytes: &[u8] = match mode {
        Mode::Light => include_bytes!("../../Metabook.app/Contents/Resources/AppIcon.icns"),
        Mode::Dark => include_bytes!("../../Metabook.app/Contents/Resources/AppIconDark.icns"),
    };
    // SAFETY: AppKit runs on the main thread, NSData copies static valid ICNS
    // bytes, and NSApplication retains the image before its autorelease.
    unsafe {
        let data = NSData::dataWithBytes_length_(nil, bytes.as_ptr().cast(), bytes.len() as u64);
        let image = NSImage::initWithData_(NSImage::alloc(nil), data);
        if image != nil {
            NSApp().setApplicationIconImage_(image);
            image.autorelease();
        }
    }
}
