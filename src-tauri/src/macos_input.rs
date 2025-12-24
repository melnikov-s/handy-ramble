//! Native macOS keyboard simulation using CGEventPost.
//!
//! This module provides a more reliable way to simulate keyboard events
//! on macOS compared to enigo, as it posts events directly to the system
//! event stream without going through the accessibility API in the same way.

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use log::debug;
use std::thread;
use std::time::Duration;

/// macOS virtual key codes
const KEY_V: CGKeyCode = 9;
const KEY_C: CGKeyCode = 8;

/// Send Cmd+V paste using CGEventPost.
/// This bypasses enigo and posts events directly to the system.
pub fn send_paste_cmd_v() -> Result<(), String> {
    debug!("[CGEvent] Sending Cmd+V paste");

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Failed to create CGEventSource")?;

    // Create key down event for 'V'
    let key_down = CGEvent::new_keyboard_event(source.clone(), KEY_V, true)
        .map_err(|_| "Failed to create key down event")?;

    // Create key up event for 'V'
    let key_up = CGEvent::new_keyboard_event(source.clone(), KEY_V, false)
        .map_err(|_| "Failed to create key up event")?;

    // Set Command modifier flag
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);

    // Post the events
    key_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(20));
    key_up.post(CGEventTapLocation::HID);

    // Small delay to let the paste complete
    thread::sleep(Duration::from_millis(50));

    debug!("[CGEvent] Cmd+V paste completed");
    Ok(())
}

/// Send Cmd+C copy using CGEventPost.
pub fn send_copy_cmd_c() -> Result<(), String> {
    debug!("[CGEvent] Sending Cmd+C copy");

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Failed to create CGEventSource")?;

    // Create key down event for 'C'
    let key_down = CGEvent::new_keyboard_event(source.clone(), KEY_C, true)
        .map_err(|_| "Failed to create key down event")?;

    // Create key up event for 'C'
    let key_up = CGEvent::new_keyboard_event(source.clone(), KEY_C, false)
        .map_err(|_| "Failed to create key up event")?;

    // Set Command modifier flag
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);

    // Post the events
    key_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(20));
    key_up.post(CGEventTapLocation::HID);

    // Wait for copy to complete
    thread::sleep(Duration::from_millis(50));

    debug!("[CGEvent] Cmd+C copy completed");
    Ok(())
}
