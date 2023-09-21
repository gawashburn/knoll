///! Concrete implementation of the display traits using macOS APIs.
///
/// Notes:
/// I Experimented with with the `CGDisplayMode` Core Graphics APIs rather
/// than using the private `CGSConfigureDisplayMode` APIs.  I encountered
/// puzzling behavior where the modes reported by `CGDisplayCopyAllDisplayModes`
/// would not include the mode reported by `CGDisplayCopyDisplayMode`.  It is
/// possible that perhaps I have not defined the FFI bindings quite correctly,
/// but for the time being the behavior of the private APIs seems closer to
/// the desired functionality.  
use log::*;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};

use crate::core_graphics::*;
use crate::displays::*;

/// Helper for converting a `CGError` with a context string into a
/// `display::Error`.  Should not be used when `CGError` is `success`.
pub fn cg_error_to_error(cg_error: CGError, context: &str) -> Error {
    assert_ne!(cg_error, CGError::success);

    Error::Internal(String::from(context))
}

/// Helper to lift a `CGError` to an `display::Error` by providing some additional
/// context as to what operation caused it.
pub fn cg_error_to_result(cg_error: CGError, context: &str) -> Result<(), Error> {
    match cg_error {
        CGError::success => Ok(()),
        _ => Err(cg_error_to_error(cg_error, context)),
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Serialize)]
pub struct RealDisplayMode {
    /// DisplayID to which this mode corresponds.
    #[serde(skip_serializing)]
    display_id: DisplayID,
    /// Internal id for this specific mode.
    #[serde(skip_serializing)]
    mode: i32,
    pub scaled: bool,
    pub color_depth: usize,
    pub frequency: usize,
    pub extents: Point,
}

impl PartialEq for RealDisplayMode {
    fn eq(&self, other: &Self) -> bool {
        self.scaled() == other.scaled()
            && self.color_depth == other.color_depth
            && self.frequency == other.frequency
            && self.extents == other.extents
    }
}

impl Eq for RealDisplayMode {}

/// TODO Only for debugging modes with seemingly identical properties.
impl Hash for RealDisplayMode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.scaled.hash(state);
        self.color_depth.hash(state);
        self.frequency.hash(state);
        self.extents.hash(state);
    }
}

impl RealDisplayMode {
    /// Create a RealDisplayMode from a `DisplayID` and `core_graphics`
    /// mode description.
    fn new(display_id: DisplayID, mode_desc: CGSDisplayModeDescription) -> Self {
        assert!(mode_desc.depth > 0, "Invalid color depth");
        assert!(mode_desc.freq > 0, "Invalid frequency");

        RealDisplayMode {
            display_id,
            mode: mode_desc.mode,
            scaled: mode_desc.scale > 1.0,
            color_depth: mode_desc.depth as usize,
            frequency: mode_desc.freq as usize,
            extents: Point {
                x: mode_desc.width as i64,
                y: mode_desc.height as i64,
            },
        }
    }
}

impl DisplayMode for RealDisplayMode {
    fn scaled(&self) -> bool {
        self.scaled
    }
    fn color_depth(&self) -> usize {
        self.color_depth
    }
    fn frequency(&self) -> usize {
        self.frequency
    }
    fn extents(&self) -> &Point {
        &self.extents
    }
}

////////////////////////////////////////////////////////////////////////////////

pub struct RealDisplayConfigTransaction {
    /// Map from UUIDs to their `DisplayID`s.
    displays: BTreeMap<String, DisplayID>,
    /// Keep track of requested rotations, so that if
    /// other configuration steps fail or the configuration is cancelled,
    /// they will not be applied.  This is not strictly necessary, but
    /// it presents a more uniform behavior for the interface.
    rotations: HashMap<DisplayID, Rotation>,
    /// The active configuration reference for this transaction.
    config_ref: CGDisplayConfigRef,
    /// Keep track whether the transaction has been dropped.
    dropped: bool,
}

impl RealDisplayConfigTransaction {
    fn new(real_display_map: &BTreeMap<String, RealDisplay>) -> Result<Self, Error> {
        let config_ref = cg_begin_display_configuration().map_err(|cg_error| {
            cg_error_to_error(
                cg_error,
                "While attempting begin a configuration transaction",
            )
        })?;

        // Documentation seems to indicate that it should be possible to
        // configure a fade for the configuration change, but it always seems
        // to fail with `notImplemented`.
        // cg_error_to_result(
        //     cg_configure_display_fade_effect(&config_ref, 0.3, 0.5, 0.0, 0.0, 0.0),
        //     "Failure configuring the fade effect",
        // )?;

        Ok(Self {
            displays: real_display_map
                .iter()
                .map(|(uuid, real_display)| (uuid.clone(), real_display.display_id))
                .collect(),
            rotations: HashMap::new(),
            config_ref,
            dropped: false,
        })
    }

    /// Helper to abstract out some of boilerplate of mapping a UUID to a
    /// display id.
    fn display_id(&self, uuid: &str) -> Result<DisplayID, Error> {
        self.displays
            .get(uuid)
            .cloned()
            .ok_or(Error::UnknownUUID(String::from(uuid)))
    }

    /// Cleaning up after beginning configuration will consume the
    /// `CGDisplayConfigRef`.  However, that requires a move.  As we
    /// cannot move the `config` field in `drop`, `complete`, and `cancel`
    /// we swap it out instead.
    fn move_config(&mut self) -> CGDisplayConfigRef {
        let mut config_ref = std::ptr::null_mut();
        std::mem::swap(&mut config_ref, &mut self.config_ref);
        config_ref
    }
}

impl DisplayConfigTransaction for RealDisplayConfigTransaction {
    type DisplayModeType = RealDisplayMode;

    fn set_mode(&mut self, uuid: &str, mode: &Self::DisplayModeType) -> Result<(), Error> {
        if self.dropped {
            return Err(Error::InvalidTransactionState);
        }

        let display_id = self.display_id(uuid)?;
        // Check that we were not passed a mode for a different display.
        // Panic here as this is programming error.
        if mode.display_id != display_id {
            panic!(
                "Tried using a display mode for display {:?} with display {:?}",
                mode.display_id, display_id
            );
        }
        cg_error_to_result(
            cgs_configure_display_mode(&self.config_ref, display_id, mode.mode),
            format!("While attempting to set the mode of {}", uuid,).as_str(),
        )
    }

    fn set_rotation(&mut self, uuid: &str, rotation: Rotation) -> Result<(), Error> {
        if self.dropped {
            return Err(Error::InvalidTransactionState);
        }

        let display_id = self.display_id(uuid)?;

        if self.rotations.contains_key(&display_id) {
            return Err(Error::DuplicateConfiguration(String::from(uuid)));
        }

        // Keep track of applied rotations and queue them up, so that
        // the overall configuration fails or is cancelled, we do not
        // apply them.
        self.rotations.insert(display_id, rotation);

        Ok(())
    }

    fn set_origin(&mut self, uuid: &str, point: &Point) -> Result<(), Error> {
        if self.dropped {
            return Err(Error::InvalidTransactionState);
        }

        let display_id = self.display_id(uuid)?;
        cg_error_to_result(
            cg_configure_display_origin(
                &self.config_ref,
                display_id,
                point.x as i32,
                point.y as i32,
            ),
            format!("While attempting to set the origin of {}", uuid).as_str(),
        )
    }

    fn set_enabled(&mut self, uuid: &str, enabled: bool) -> Result<(), Error> {
        if self.dropped {
            return Err(Error::InvalidTransactionState);
        }

        let display_id = self.display_id(uuid)?;
        if !enabled {
            cg_error_to_result(
                cgs_configure_display_enabled(&self.config_ref, display_id, enabled),
                format!("While attempting to adjust the enablement of {}", uuid).as_str(),
            )
        } else {
            Ok(())
        }
    }

    fn cancel(mut self) -> Result<(), Error> {
        if self.dropped {
            return Err(Error::InvalidTransactionState);
        }

        cg_error_to_result(
            cg_cancel_display_configuration(self.move_config()),
            "While attempting to cancel the configuration transaction",
        )?;
        self.dropped = true;
        Ok(())
    }

    fn commit(mut self) -> Result<(), Error> {
        if self.dropped {
            return Err(Error::InvalidTransactionState);
        }

        cg_error_to_result(
            cg_complete_display_configuration(
                self.move_config(),
                CGConfigureOption::kCGConfigurePermanently,
            ),
            "While attempting to commit the configuration transaction",
        )?;

        for (&display_id, &rotation) in &self.rotations {
            mpd_set_rotation(display_id, rotation as i32)
        }

        self.dropped = true;
        Ok(())
    }
}

impl Drop for RealDisplayConfigTransaction {
    /// Ensure that we consume the `CGDisplayConfigRef` if an API consumer
    /// fails to call `complete` or `cancel`.
    fn drop(&mut self) {
        if !self.dropped {
            cg_cancel_display_configuration(self.move_config());
            self.dropped = true;
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct RealDisplay {
    /// DisplayID used to associate this RealDisplay with an attached display.
    display_id: DisplayID,
    uuid: String,
    enabled: bool,
    origin: Point,
    rotation: Rotation,
    mode: RealDisplayMode,
    modes: Vec<RealDisplayMode>,
}

/// Undo display_rotation to the Point.  Note that this is not
/// the same thing as rotating the Point in 2D space.  This is just
/// so that display extents can be presented uniformly in landscape
/// resolution.
pub fn undo_display_rotation(point: Point, rotation: Rotation) -> Point {
    match rotation {
        Rotation::Zero | Rotation::OneEighty => point,
        Rotation::Ninety | Rotation::TwoSeventy => Point {
            x: point.y,
            y: point.x,
        },
    }
}

impl RealDisplay {
    /// Obtain a unique identifying name for the given display.
    /// TODO Perform some additional testing to see this remains "persistent"
    /// for identical model displays.
    fn compute_uuid(display_id: DisplayID) -> String {
        // Use CoreGraphics UUID API.  I've already determined that for
        // some of dual displays that the manufacturer doesn't report a
        // meaningful serial number.
        let cfuuid = cg_display_create_uuid_from_display_id(display_id);
        let cfstring = cf_uuid_create_string(kCFAllocatorDefault, cfuuid);
        let mut buffer: [u8; 37] = [0; 37];
        if !cf_string_get_cstring(
            cfstring,
            &mut buffer,
            CFStringBuiltInEncodings::ASCII as CFStringEncoding,
        ) {
            // It seems reasonable to panic here, as the UUID has a fixed
            // format and length.
            panic!("Buffer to receive UUID is too small.")
        }
        // Need to manually release the resources.
        cf_release(cfstring);
        cf_release(cfuuid);

        // It is safe to unwrap here as we know that the UUID will always
        // be ASCII which should never fail with from_utf8.
        String::from_utf8(buffer[0..36].to_vec())
            .unwrap()
            .to_lowercase()
            .replace('-', "")
    }

    /// Create a `RealDisplay` given a `DisplayID`.
    fn new(display_id: DisplayID) -> Result<Self, Error> {
        let uuid = RealDisplay::compute_uuid(display_id);

        let mut num_modes = 0;
        cg_error_to_result(
            cgs_get_number_of_display_modes(display_id, &mut num_modes),
            format!(
                "While attempting to obtain the number of display modes on {}",
                uuid
            )
            .as_str(),
        )?;

        // Obtain the current display rotation for normalizing the modes.
        let float_rotation = cg_display_rotation(display_id);
        let rotation = Rotation::from(float_rotation)
            .expect(format!("Unexpected display rotation angle: {}", float_rotation).as_str());

        let mut current_mode_num = 0;
        cg_error_to_result(
            cgs_get_current_display_mode(display_id, &mut current_mode_num),
            format!(
                "While attempting to obtain the current display mode on {}",
                uuid
            )
            .as_str(),
        )?;
        let mut current_mode = None;

        // Temporary for debugging
        let mut mode_buckets: HashMap<RealDisplayMode, Vec<CGSDisplayModeDescription>> =
            HashMap::new();

        let mut possible_modes = Vec::new();
        for mode_num in 0..num_modes {
            let mut desc = CGSDisplayModeDescription::default();
            cg_error_to_result(
                cgs_get_display_mode_description(display_id, mode_num, &mut desc),
                format!("While attempting to obtain a mode description on {}", uuid).as_str(),
            )?;

            // TODO Eliminate clone
            let mut mode = RealDisplayMode::new(display_id, desc.clone());
            // Normalize the extents.
            mode.extents = undo_display_rotation(mode.extents, rotation);

            if current_mode_num == mode_num {
                current_mode = Some(mode.clone());
            }

            // Group mode descriptions into buckets for investigation.
            match mode_buckets.get_mut(&mode) {
                Some(descs) => descs.push(desc),
                None => {
                    mode_buckets.insert(mode.clone(), vec![desc]);
                }
            }

            possible_modes.push(mode);
        }

        // Log the duplicates.
        // Further investigation is needed as to why some essentially duplicate
        // modes are reported from the API.
        for (mode, descs) in &mode_buckets {
            if descs.len() > 1 {
                warn!(
                    "Encountered display modes with identical properties {:?}:  {:?}",
                    mode, descs
                );
            }
        }

        // TODO Is the likely enough that it should be reported as an actual
        //   error condition?
        assert!(current_mode.is_some());

        let enabled = cg_display_is_active(display_id) || cg_display_is_in_mirror_set(display_id);
        let cg_point = cg_display_bounds(display_id).origin;

        Ok(RealDisplay {
            display_id,
            uuid,
            enabled,
            origin: Point {
                x: cg_point.x as i64,
                y: cg_point.y as i64,
            },
            rotation,
            mode: current_mode.unwrap(),
            modes: mode_buckets.into_keys().collect::<Vec<RealDisplayMode>>(),
        })
    }
}

impl Display for RealDisplay {
    fn uuid(&self) -> &str {
        self.uuid.as_str()
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn origin(&self) -> &Point {
        &self.origin
    }

    fn rotation(&self) -> Rotation {
        self.rotation
    }

    type DisplayModeType = RealDisplayMode;

    fn current_mode(&self) -> &Self::DisplayModeType {
        &self.mode
    }

    fn possible_modes(&self) -> &[Self::DisplayModeType] {
        &self.modes.as_slice()
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct RealDisplayState {
    displays: BTreeMap<String, RealDisplay>,
}

impl DisplayState for RealDisplayState {
    fn current() -> Result<Self, Error> {
        // The current Mac Pro supports eight monitors:
        // https://support.apple.com/en-us/HT213665
        // I have seen references to twelve being supported on some models.
        // So I feel for now 64 is a reasonable bound on real monitors.
        // I will need to experiment with if/how Sidecar displays respond
        // to these APIs.
        let mut display_ids: [DisplayID; 64] = [DisplayID::default(); 64];
        let mut num_displays: u32 = 0;
        // We want the online rather than active displays as that will not
        // include mirrored or sleeping displays.
        cg_get_online_display_list(&mut display_ids, &mut num_displays);
        assert!(
            num_displays <= 64,
            "Number of displays is more than the input array."
        );

        let mut displays = Vec::new();
        for id in display_ids.into_iter().take(num_displays as usize) {
            displays.push(RealDisplay::new(id)?);
        }

        Ok(RealDisplayState {
            displays: displays
                .into_iter()
                .map(|d: RealDisplay| (d.uuid.clone(), d))
                .collect(),
        })
    }

    type DisplayModeType = RealDisplayMode;
    type DisplayType = RealDisplay;
    type DisplayConfigTransactionType = RealDisplayConfigTransaction;

    fn get_displays(&self) -> &BTreeMap<String, Self::DisplayType> {
        &self.displays
    }

    fn configure(&self) -> Result<Self::DisplayConfigTransactionType, Error> {
        RealDisplayConfigTransaction::new(&self.displays)
    }
}
