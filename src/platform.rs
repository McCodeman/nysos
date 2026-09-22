// SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
// SPDX-License-Identifier: Apache-2.0

/// SGR mouse reports carry Shift/Alt/Ctrl, but omit Command. For a local
/// macOS session, sample only modifier flags when a click arrives. This does
/// not install an event tap or read keyboard contents. Remote sessions must
/// use the modifiers supplied by their terminal (Alt-click). A tmux server can
/// outlive its original local client, so its environment cannot prove locality.
#[cfg(target_os = "macos")]
pub fn command_pressed() -> bool {
    if std::env::var_os("SSH_CONNECTION").is_some()
        || std::env::var_os("SSH_TTY").is_some()
        || std::env::var_os("TMUX").is_some()
    {
        return false;
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceFlagsState(state_id: i32) -> u64;
    }
    // SAFETY: CoreGraphics takes a scalar enum value (combined session = 0)
    // and returns a bitmask. It has no pointer or lifetime preconditions.
    let flags = unsafe { CGEventSourceFlagsState(0) };
    flags & (1 << 20) != 0
}

#[cfg(not(target_os = "macos"))]
pub fn command_pressed() -> bool {
    false
}
