#![allow(dead_code)]
///! This module provides "safer" access to all of the low-level macOS APIs.  
///
/// I suppose I could have considered using the core_graphics crate
/// (https://docs.rs/core-graphics/latest/core_graphics), but it doesn't
/// currently provide "safe" versions of some of these functions, doesn't
/// provide some of the public APIs and none of the private APIs at all.
/// So it would have only saved me some of the extern declarations.
use std::os::raw::c_int;
use std::os::raw::c_void;
use std::ptr::{null, null_mut};

use objc::runtime::Object;
use static_assertions::const_assert;

////////////////////////////////////////////////////////////////////////////////
// Types common to the unsafe and safe APIs

type CGDirectDisplayID = u32;

/// Wrapper so that we do not need to expose the actual implementation.
#[derive(Debug, Default, Hash, Eq, PartialEq, Clone, Copy)]
#[repr(C)]
pub struct DisplayID {
    id: CGDirectDisplayID,
}

/// https://github.com/NUIKit/CGSInternal/blob/master/CGSDisplays.h
#[derive(Debug, Default, Clone)]
#[repr(C)]
pub struct CGSDisplayModeDescription {
    pub mode: i32,
    flags: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    // This is split into two separate fields as compared CGSDisplays.h
    // because derive(Default) cannot handle arrays 42 elements long.
    // Switching to u64 will change the struct layout.
    dc2: [u32; 21],
    dc3: [u32; 21],
    dc4: u16,
    pub freq: u16,
    dc5: [u8; 16], // dc5[4] seems to also contain the same value as mode.
    // If 2.0, the mode is scaled.
    pub scale: f32,
}

// Sanity check that the struct has the expected size.
const_assert!(std::mem::size_of::<CGSDisplayModeDescription>() == 0xD4);

/// https://developer.apple.com/documentation/corefoundation/cgpoint
#[derive(Debug)]
#[repr(C)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

/// https://developer.apple.com/documentation/corefoundation/cgrect
#[derive(Debug)]
#[repr(C)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGPoint,
}

/// https://developer.apple.com/documentation/coregraphics/cgerror/
#[derive(Debug, Eq, PartialEq)]
#[repr(i32)]
#[allow(non_camel_case_types)]
pub enum CGError {
    cannotComplete = 1004,
    failure = 1000,
    illegalArgument = 1001,
    invalidConnection = 1002,
    invalidContext = 1003,
    invalidOperation = 1010,
    noneAvailable = 1011,
    notImplemented = 1006,
    rangeCheck = 1007,
    success = 0,
    typeCheck = 1008,
}

// https://developer.apple.com/documentation/coregraphics/cgconfigureoption
#[derive(Debug)]
#[repr(i32)]
#[allow(non_camel_case_types)]
pub enum CGConfigureOption {
    // Currently not used but included for possible future use.
    #[allow(unused)]
    kCGConfigureForAppOnly = 0,
    // Currently not used but included for possible future use.
    #[allow(unused)]
    kCGConfigureForSession = 1,
    kCGConfigurePermanently = 2,
}

#[allow(non_upper_case_globals)]
pub const kCFAllocatorDefault: *const c_void = null();

// TODO I could consider adding wrapper struct to hide implementations
//   of some of these types, but as nearly all of these are opaque pointers,
//   the likelihood of forging meaningful values that do not immediately
//   cause a segfault seems unlikely.

pub type CGDisplayConfigRef = *mut c_void;
pub type CGDisplayModeRef = *const c_void;
pub type CGDisplayFadeInterval = f32;

/// Wrapper over CFArray.
pub type CGDisplayModeArray = *const c_void;

pub type CFArray = *const c_void;
pub type CFTypeRef = *const c_void;
pub type CFAllocator = *const c_void;
pub type CFUUID = *const c_void;
pub type CFIndex = c_int;
pub type CFString = *const c_void;
pub type CFDictionary = *const c_void;
pub type CFStringEncoding = u32;

// https://developer.apple.com/documentation/corefoundation/cfstringbuiltinencodings
#[derive(Debug)]
#[repr(i32)]
#[allow(non_camel_case_types)]
pub enum CFStringBuiltInEncodings {
    ASCII = 1536,
    // Currently not used but included for possible future use.
    UTF8 = 134217984,
}

/// Implement From to expose a safer conversion.
impl From<CFStringBuiltInEncodings> for CFStringEncoding {
    fn from(value: CFStringBuiltInEncodings) -> Self {
        value as Self
    }
}

// TODO Add values or remove these constants ?
// kDisplayProductIDGeneric
// kDisplayVendorIDUnknown

#[link(name = "AppKit", kind = "framework")]
#[link(name = "IOKit", kind = "framework")]
#[link(name = "ApplicationServices", kind = "framework")]
#[link(name = "DisplayServices", kind = "framework")]
#[link(name = "CoreDisplay", kind = "framework")]
#[link(name = "OSD", kind = "framework")]
#[link(name = "MonitorPanel", kind = "framework")]
#[link(name = "SkyLight", kind = "framework")]
unsafe extern "C" {
    /// https://developer.apple.com/documentation/corefoundation/1521153-cfrelease
    fn CFRelease(cf: CFTypeRef);

    /// https://developer.apple.com/documentation/corefoundation/1542721-cfstringgetcstring/
    fn CFStringGetCString(
        string: CFString,
        buffer: *mut u8,
        buffer_size: CFIndex,
        encoding: CFStringEncoding,
    ) -> bool;

    /// https://developer.apple.com/documentation/corefoundation/1543625-cfuuidcreatestring/
    fn CFUUIDCreateString(allocator: CFAllocator, uuid: CFUUID) -> CFString;

    /// https://developer.apple.com/documentation/corefoundation/1388772-cfarraygetcount/
    fn CFArrayGetCount(array: CFArray) -> usize;

    /// https://developer.apple.com/documentation/corefoundation/1388767-cfarraygetvalueatindex/
    fn CFArrayGetValueAtIndex(array: CFArray, idx: CFIndex) -> *const c_void;

    /// https://developer.apple.com/documentation/coregraphics/1454964-cggetonlinedisplaylist/
    fn CGGetOnlineDisplayList(
        max_displays: u32,
        online_displays: *mut CGDirectDisplayID,
        display_count: *mut u32,
    ) -> CGError;

    /// https://developer.apple.com/documentation/coregraphics/1454603-cggetactivedisplaylist/
    fn CGGetActiveDisplayList(
        max_displays: u32,
        active_displays: *mut CGDirectDisplayID,
        display_count: *mut u32,
    ) -> CGError;

    /// https://developer.apple.com/documentation/coregraphics/1455409-cgdisplayserialnumber
    /// Returns 0x0 if there is no encoded serial number or it cannot be identified
    fn CGDisplaySerialNumber(display_id: CGDirectDisplayID) -> u32;

    /// https://developer.apple.com/documentation/coregraphics/1454149-cgdisplaymodelnumber
    /// Returns kDisplayProductIDGeneric if it cannot be identified.
    fn CGDisplayModelNumber(display_id: CGDirectDisplayID) -> u32;

    /// https://developer.apple.com/documentation/coregraphics/1455233-cgdisplayvendornumber
    /// Returns kDisplayVendorIDUnknown if the vendor cannot be identified.
    fn CGDisplayVendorNumber(display_id: CGDirectDisplayID) -> u32;

    /// https://developer.apple.com/documentation/coregraphics/1455299-cgdisplayrotation
    fn CGDisplayRotation(display_id: CGDirectDisplayID) -> f64;

    /// https://developer.apple.com/documentation/coregraphics/1456395-cgdisplaybounds/
    fn CGDisplayBounds(display_id: CGDirectDisplayID) -> CGRect;

    /// https://developer.apple.com/documentation/coregraphics/1455222-cgdisplayisactive
    fn CGDisplayIsActive(display_id: CGDirectDisplayID) -> bool;

    /// https://developer.apple.com/documentation/coregraphics/1455558-cgdisplayisinmirrorset
    fn CGDisplayIsInMirrorSet(display_id: CGDirectDisplayID) -> bool;

    /// https://developer.apple.com/documentation/colorsync/1458801-cgdisplaycreateuuidfromdisplayid/
    fn CGDisplayCreateUUIDFromDisplayID(display_id: CGDirectDisplayID) -> CFUUID;

    /// https://developer.apple.com/documentation/coregraphics/1562069-cgdisplaymoderelease
    fn CGDisplayModeRelease(mode: CGDisplayModeRef);

    /// https://developer.apple.com/documentation/coregraphics/1455537-cgdisplaycopyalldisplaymodes
    fn CGDisplayCopyAllDisplayModes(
        display_id: CGDirectDisplayID,
        options: CFDictionary,
    ) -> CFArray;

    /// https://developer.apple.com/documentation/coregraphics/1454099-cgdisplaycopydisplaymode
    /// Returns null pointer for invalid input display.  Caller is response for releasing.
    fn CGDisplayCopyDisplayMode(display_id: CGDirectDisplayID) -> CGDisplayModeRef;

    /// https://developer.apple.com/documentation/coregraphics/1454442-cgdisplaymodegetwidth/
    fn CGDisplayModeGetWidth(mode: CGDisplayModeRef) -> usize;

    /// https://developer.apple.com/documentation/coregraphics/1455380-cgdisplaymodegetheight
    fn CGDisplayModeGetHeight(mode: CGDisplayModeRef) -> usize;

    /// https://developer.apple.com/documentation/coregraphics/1454661-cgdisplaymodegetrefreshrate
    fn CGDisplayModeGetRefreshRate(mode: CGDisplayModeRef) -> f64;

    /// https://developer.apple.com/documentation/coregraphics/1454092-cgdisplaymodegetioflags
    fn CGDisplayModeGetIOFlags(mode: CGDisplayModeRef) -> u32;

    /// https://developer.apple.com/documentation/coregraphics/1454728-cgdisplaymodegetiodisplaymodeid
    /// These mode numbers do appear to correspond to those reported by
    /// `CGSGetCurrentDisplayMode`, etc.
    fn CGDisplayModeGetIODisplayModeID(mode: CGDisplayModeRef) -> i32;

    /// https://developer.apple.com/documentation/coregraphics/1454756-cgdisplaymodegetpixelwidth/
    fn CGDisplayModeGetPixelWidth(mode: CGDisplayModeRef) -> usize;

    /// https://developer.apple.com/documentation/coregraphics/1456406-cgdisplaymodegetpixelheight/
    fn CGDisplayModeGetPixelHeight(mode: CGDisplayModeRef) -> usize;

    /// https://developer.apple.com/documentation/coregraphics/cgdisplaymode/1454928-isusablefordesktopgui
    fn CGDisplayModeIsUsableForDesktopGUI(mode: CGDisplayModeRef) -> bool;

    /// https://developer.apple.com/documentation/coregraphics/1455235-cgbegindisplayconfiguration/
    /// Pointer allowed to contain garbage on input?
    fn CGBeginDisplayConfiguration(config: *mut CGDisplayConfigRef) -> CGError;

    /// https://developer.apple.com/documentation/coregraphics/1454488-cgcompletedisplayconfiguration/
    fn CGCompleteDisplayConfiguration(
        config: CGDisplayConfigRef,
        option: CGConfigureOption,
    ) -> CGError;

    /// https://developer.apple.com/documentation/coregraphics/1455522-cgcanceldisplayconfiguration
    fn CGCancelDisplayConfiguration(config: CGDisplayConfigRef) -> CGError;

    /// https://developer.apple.com/documentation/coregraphics/1454103-cgconfiguredisplayfadeeffect
    fn CGConfigureDisplayFadeEffect(
        config: CGDisplayConfigRef,
        fade_out_seconds: CGDisplayFadeInterval,
        fade_in_seconds: CGDisplayFadeInterval,
        fade_red: f32,
        fade_green: f32,
        fade_blue: f32,
    ) -> CGError;

    /// https://developer.apple.com/documentation/coregraphics/1454090-cgconfiguredisplayorigin/
    /// Setting the origin to 0,0 will make the display the main.
    /// Doesn't consume ref, so take reference
    fn CGConfigureDisplayOrigin(
        config: CGDisplayConfigRef,
        display_id: CGDirectDisplayID,
        x: i32,
        y: i32,
    ) -> CGError;

    /// https://developer.apple.com/documentation/coregraphics/1454273-cgconfiguredisplaywithdisplaymod
    /// options is reserved for future use and should always be null at present.
    fn CGConfigureDisplayWithDisplayMode(
        config: CGDisplayConfigRef,
        display_id: CGDirectDisplayID,
        mode: CGDisplayModeRef,
        options: CFDictionary,
    ) -> CGError;

    /// https://developer.apple.com/documentation/coregraphics/1454531-cgconfiguredisplaymirrorofdispla
    /// Doesn't consume ref, so take reference
    fn CGConfigureDisplayMirrorOfDisplay(
        config: CGDisplayConfigRef,
        display_id: CGDirectDisplayID,
        master_id: CGDirectDisplayID,
    ) -> CGError;

    /// https://developer.apple.com/documentation/coregraphics/1455336-cgdisplayregisterreconfiguration
    fn CGDisplayRegisterReconfigurationCallback(
        callback: extern "C" fn(),
        user_info: *mut c_void,
    ) -> CGError;

    // TODO Callbacks do not appear to leak if process exits, but might good
    //   to clean up?  Need to determine appropriate location to call.
    // CGDisplayRemoveReconfigurationCallback

    /// https://developer.apple.com/documentation/coregraphics/cgdisplaymirrorsdisplay(_:)
    fn CGDisplayMirrorsDisplay(display_id: CGDirectDisplayID) -> CGDirectDisplayID;

    /// https://developer.apple.com/documentation/appkit/1428475-nsapplicationload
    pub fn NSApplicationLoad() -> bool;
    /// https://developer.apple.com/documentation/corefoundation/1542011-cfrunlooprun/
    pub fn CFRunLoopRun();

    // Private Core Graphics APIs ////////////////////////////////////////////
    // https://github.com/NUIKit/CGSInternal/blob/master/CGSDisplays.h

    fn CGSGetCurrentDisplayMode(display: CGDirectDisplayID, mode_num: *mut c_int) -> CGError;

    fn CGSGetNumberOfDisplayModes(display: CGDirectDisplayID, num_modes: *mut c_int) -> CGError;

    fn CGSGetDisplayModeDescriptionOfLength(
        display_id: CGDirectDisplayID,
        index: c_int,
        mode: *mut CGSDisplayModeDescription,
        length: c_int,
    ) -> CGError;

    // Suspect these do not consume their arguments.

    fn CGSConfigureDisplayMode(
        config: CGDisplayConfigRef,
        display_id: CGDirectDisplayID,
        mode_num: c_int,
    ) -> CGError;

    fn CGSConfigureDisplayEnabled(
        config: CGDisplayConfigRef,
        display_id: CGDirectDisplayID,
        enabled: bool,
    ) -> CGError;

}

////////////////////////////////////////////////////////////////////////////////

pub fn cf_release(cf: CFTypeRef) {
    unsafe { CFRelease(cf) }
}

pub fn cf_string_get_cstring(
    string: CFString,
    buffer: &mut [u8],
    encoding: CFStringEncoding,
) -> bool {
    unsafe { CFStringGetCString(string, buffer.as_mut_ptr(), buffer.len() as c_int, encoding) }
}

pub fn cf_uuid_create_string(allocator: CFAllocator, uuid: CFUUID) -> CFString {
    unsafe { CFUUIDCreateString(allocator, uuid) }
}

pub fn cg_get_online_display_list(
    online_displays: &mut [DisplayID],
    display_count: &mut u32,
) -> CGError {
    unsafe {
        CGGetOnlineDisplayList(
            online_displays.len() as u32,
            online_displays.as_mut_ptr() as *mut CGDirectDisplayID,
            display_count,
        )
    }
}

pub fn cg_get_active_display_list(
    active_displays: &mut [DisplayID],
    display_count: &mut u32,
) -> CGError {
    unsafe {
        CGGetActiveDisplayList(
            active_displays.len() as u32,
            active_displays.as_mut_ptr() as *mut CGDirectDisplayID,
            display_count,
        )
    }
}

pub fn cg_display_serial_number(display_id: DisplayID) -> u32 {
    unsafe { CGDisplaySerialNumber(display_id.id) }
}

pub fn cg_display_model_number(display_id: DisplayID) -> u32 {
    unsafe { CGDisplayModelNumber(display_id.id) }
}

pub fn cg_display_vendor_number(display_id: DisplayID) -> u32 {
    unsafe { CGDisplayModelNumber(display_id.id) }
}

pub fn cg_display_rotation(display_id: DisplayID) -> f64 {
    unsafe { CGDisplayRotation(display_id.id) }
}

pub fn cg_display_bounds(display_id: DisplayID) -> CGRect {
    unsafe { CGDisplayBounds(display_id.id) }
}

pub fn cg_display_is_active(display_id: DisplayID) -> bool {
    unsafe { CGDisplayIsActive(display_id.id) }
}

pub fn cg_display_is_in_mirror_set(display_id: DisplayID) -> bool {
    unsafe { CGDisplayIsInMirrorSet(display_id.id) }
}

pub fn cg_display_create_uuid_from_display_id(display_id: DisplayID) -> CFUUID {
    unsafe { CGDisplayCreateUUIDFromDisplayID(display_id.id) }
}

pub fn cg_display_mode_release(mode: CGDisplayModeRef) {
    unsafe { CGDisplayModeRelease(mode) }
}

pub fn cg_display_copy_all_display_modes(display_id: DisplayID) -> CGDisplayModeArray {
    unsafe { CGDisplayCopyAllDisplayModes(display_id.id, null()) }
}

// Derived helpers
pub fn cg_display_modes_get_count(modes: CGDisplayModeArray) -> usize {
    unsafe { CFArrayGetCount(modes) }
}

pub fn cg_display_modes_at_index(modes: CGDisplayModeArray, idx: CFIndex) -> CGDisplayModeRef {
    unsafe { CFArrayGetValueAtIndex(modes, idx) }
}

pub fn cg_display_copy_display_mode(display_id: DisplayID) -> Option<CGDisplayModeRef> {
    unsafe {
        let mode = CGDisplayCopyDisplayMode(display_id.id);
        if !mode.is_null() { Some(mode) } else { None }
    }
}

pub fn cg_display_mode_get_width(mode: &CGDisplayModeRef) -> usize {
    unsafe { CGDisplayModeGetWidth(*mode) }
}

pub fn cg_display_mode_get_height(mode: &CGDisplayModeRef) -> usize {
    unsafe { CGDisplayModeGetHeight(*mode) }
}

pub fn cg_display_mode_get_pixel_width(mode: &CGDisplayModeRef) -> usize {
    unsafe { CGDisplayModeGetPixelWidth(*mode) }
}

pub fn cg_display_mode_get_pixel_height(mode: &CGDisplayModeRef) -> usize {
    unsafe { CGDisplayModeGetPixelHeight(*mode) }
}

pub fn cg_display_mode_get_refresh_rate(mode: &CGDisplayModeRef) -> f64 {
    unsafe { CGDisplayModeGetRefreshRate(*mode) }
}

pub fn cg_display_mode_get_io_flags(mode: &CGDisplayModeRef) -> u32 {
    unsafe { CGDisplayModeGetIOFlags(*mode) }
}

pub fn cg_display_mode_get_io_display_mode_id(mode: &CGDisplayModeRef) -> i32 {
    unsafe { CGDisplayModeGetIODisplayModeID(*mode) }
}

pub fn cg_display_mode_is_usable_for_desktop_gui(mode: &CGDisplayModeRef) -> bool {
    unsafe { CGDisplayModeIsUsableForDesktopGUI(*mode) }
}

pub fn cg_begin_display_configuration() -> Result<CGDisplayConfigRef, CGError> {
    unsafe {
        let mut config_ref: CGDisplayConfigRef = null_mut();
        let error = CGBeginDisplayConfiguration(&mut config_ref);
        match error {
            CGError::success => Ok(config_ref),
            _ => Err(error),
        }
    }
}

pub fn cg_complete_display_configuration(
    config_ref: CGDisplayConfigRef,
    option: CGConfigureOption,
) -> CGError {
    unsafe { CGCompleteDisplayConfiguration(config_ref, option) }
}

pub fn cg_cancel_display_configuration(config_ref: CGDisplayConfigRef) -> CGError {
    unsafe { CGCancelDisplayConfiguration(config_ref) }
}

pub fn cg_configure_display_origin(
    config_ref: &CGDisplayConfigRef,
    display_id: DisplayID,
    x: i32,
    y: i32,
) -> CGError {
    unsafe { CGConfigureDisplayOrigin(*config_ref, display_id.id, x, y) }
}

pub fn cg_configure_display_with_display_mode(
    config_ref: &CGDisplayConfigRef,
    display_id: DisplayID,
    mode: &CGDisplayModeRef,
) -> CGError {
    unsafe { CGConfigureDisplayWithDisplayMode(*config_ref, display_id.id, *mode, null()) }
}

pub fn cg_configure_display_fade_effect(
    config_ref: &CGDisplayConfigRef,
    fade_out_seconds: CGDisplayFadeInterval,
    fade_in_seconds: CGDisplayFadeInterval,
    fade_red: f32,
    fade_green: f32,
    fade_blue: f32,
) -> CGError {
    unsafe {
        CGConfigureDisplayFadeEffect(
            *config_ref,
            fade_out_seconds,
            fade_in_seconds,
            fade_red,
            fade_green,
            fade_blue,
        )
    }
}

pub fn cg_configure_display_mirror_of_display(
    config_ref: &CGDisplayConfigRef,
    display_id: DisplayID,
    master_id: Option<DisplayID>,
) -> CGError {
    // In CGDirectDisplay.h kCGNullDirectDisplay is a defined as a
    // preprocess macro to be 0, as such there is no symbol we can
    // link to obtain the value.
    let target_id = master_id.map(|d| d.id).unwrap_or(0);

    unsafe { CGConfigureDisplayMirrorOfDisplay(*config_ref, display_id.id, target_id) }
}

pub fn cg_display_register_reconfiguration_callback(cb: extern "C" fn()) -> CGError {
    unsafe { CGDisplayRegisterReconfigurationCallback(cb, null_mut()) }
}

pub fn ns_application_load() -> bool {
    unsafe { NSApplicationLoad() }
}

pub fn cf_run_loop_run() {
    unsafe { CFRunLoopRun() }
}

pub fn cgs_get_current_display_mode(display: DisplayID, mode: &mut i32) -> CGError {
    unsafe { CGSGetCurrentDisplayMode(display.id, mode) }
}

pub fn cgs_get_number_of_display_modes(display: DisplayID, num: &mut i32) -> CGError {
    unsafe { CGSGetNumberOfDisplayModes(display.id, num) }
}

pub fn cgs_get_display_mode_description(
    display_id: DisplayID,
    index: i32,
    mode_desc: &mut CGSDisplayModeDescription,
) -> CGError {
    unsafe {
        CGSGetDisplayModeDescriptionOfLength(
            display_id.id,
            index,
            mode_desc,
            std::mem::size_of::<CGSDisplayModeDescription>() as c_int,
        )
    }
}

pub fn cgs_configure_display_mode(
    config_ref: &CGDisplayConfigRef,
    display_id: DisplayID,
    mode_num: i32,
) -> CGError {
    unsafe { CGSConfigureDisplayMode(*config_ref, display_id.id, mode_num) }
}

pub fn cgs_configure_display_enabled(
    config_ref: &CGDisplayConfigRef,
    display_id: DisplayID,
    enabled: bool,
) -> CGError {
    unsafe { CGSConfigureDisplayEnabled(*config_ref, display_id.id, enabled) }
}

pub fn cg_display_mirrors_display(display_id: DisplayID) -> Option<DisplayID> {
    let mirrored_id = unsafe { CGDisplayMirrorsDisplay(display_id.id) };
    if mirrored_id == 0 {
        None
    } else {
        Some(DisplayID { id: mirrored_id })
    }
}

/// Helper to set the rotation of a display via the MPDisplay Objective-C class.
// TODO As this is the only use of Objective-C in this code-base,
//   in the future it may be worth investigate the private
//   `CGSSetDisplayRotation` function instead.  Its existence is
//   referenced here: https://github.com/NUIKit/CGSInternal/issues/3
//   but there is no documentation of what its expected function
//   prototype would be.  So some further detective work will be
//   necessary.
pub fn mpd_set_rotation(display_id: DisplayID, rotation: i32) {
    // https://github.com/phatblat/macOSPrivateFrameworks/tree/9047371eb80f925642c8a7c4f1e00095aec66044/PrivateFrameworks/MonitorPanel
    unsafe {
        let cls = class!(MPDisplay);
        let obj: *mut Object = objc::msg_send![cls, alloc];
        assert_ne!(
            obj,
            null_mut(),
            "Received a null pointer as a result of Objective-C message send."
        );
        let _: () = objc::msg_send![obj, initWithCGSDisplayID:display_id.id];
        let _: () = objc::msg_send![obj, setOrientation:rotation];
    }
    // TODO Need to wait to confirm operation succeeded?
    //   So far I have never seen a rotation fail in practice.
}
