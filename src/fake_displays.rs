/// allow testing various aspects of knoll independent of the displays
/// actually attached to the computer.
use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::displays::*;

#[derive(Debug, Eq, PartialEq, Clone, Serialize)]
pub struct FakeDisplayMode {
    #[serde(skip_serializing)]
    pub uuid: String,
    pub scaled: bool,
    pub color_depth: usize,
    pub frequency: usize,
    pub extents: Point,
}

impl DisplayMode for FakeDisplayMode {
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

#[derive(Debug, Clone)]
enum FakeDisplayEdit {
    SetMode(FakeDisplayMode),
    SetRotation(Rotation),
    SetOrigin(Point),
    SetEnabled(bool),
}

pub struct FakeDisplayConfigTransaction {
    dropped: bool,
    edit_map: BTreeMap<String, Vec<FakeDisplayEdit>>,
}

impl FakeDisplayConfigTransaction {
    fn new(display_map: &BTreeMap<String, FakeDisplay>) -> Result<Self, Error> {
        Ok(Self {
            dropped: false,
            edit_map: display_map
                .iter()
                .map(|(uuid, _)| (uuid.clone(), Vec::new()))
                .collect(),
        })
    }

    /// Helper to eliminate the boilerplate in looking up a display
    /// and storing the requested edit.
    fn record_edit(&mut self, uuid: &str, edit: FakeDisplayEdit) -> Result<(), Error> {
        if self.dropped {
            return Err(Error::InvalidTransactionState);
        }

        match self.edit_map.get_mut(uuid) {
            Some(edits) => {
                edits.push(edit);
                Ok(())
            }
            None => Err(Error::UnknownUUID(String::from(uuid))),
        }
    }
}

impl DisplayConfigTransaction for FakeDisplayConfigTransaction {
    type DisplayModeType = FakeDisplayMode;

    fn set_mode(&mut self, uuid: &str, mode: &Self::DisplayModeType) -> Result<(), Error> {
        if mode.uuid != uuid {
            panic!(
                "Tried using a display mode for display {} with display {}",
                mode.uuid, uuid
            );
        }

        self.record_edit(uuid, FakeDisplayEdit::SetMode(mode.clone()))
    }

    fn set_rotation(&mut self, uuid: &str, rotation: Rotation) -> Result<(), Error> {
        self.record_edit(uuid, FakeDisplayEdit::SetRotation(rotation))
    }

    fn set_origin(&mut self, uuid: &str, point: &Point) -> Result<(), Error> {
        self.record_edit(uuid, FakeDisplayEdit::SetOrigin(point.clone()))
    }

    fn set_enabled(&mut self, uuid: &str, enabled: bool) -> Result<(), Error> {
        self.record_edit(uuid, FakeDisplayEdit::SetEnabled(enabled))
    }

    fn cancel(mut self) -> Result<(), Error> {
        if self.dropped {
            return Err(Error::InvalidTransactionState);
        }
        self.dropped = true;
        Ok(())
    }

    fn commit(mut self) -> Result<(), Error> {
        if self.dropped {
            return Err(Error::InvalidTransactionState);
        }
        self.dropped = true;

        let mut guard = CURRENT_FAKE_DISPLAYS
            .lock()
            .map_err(|pe| Error::Poisoned(format!("{}", pe)))?;

        // Iterate through the recorded edits applying them.
        while let Some((uuid, edits)) = self.edit_map.pop_first() {
            match guard.get_mut(&uuid) {
                Some(display) => {
                    for edit in edits {
                        display.apply_edit(edit);
                    }
                }
                None => {
                    self.dropped = true;
                    return Err(Error::UnknownUUID(String::from(uuid)));
                }
            }
        }

        Ok(())
    }
}

impl Drop for FakeDisplayConfigTransaction {
    fn drop(&mut self) {
        if !self.dropped {
            self.dropped = true;
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct FakeDisplay {
    uuid: String,
    enabled: bool,
    origin: Point,
    rotation: Rotation,
    mode: FakeDisplayMode,
    modes: Vec<FakeDisplayMode>,
}

impl FakeDisplay {
    /// Helper to apply edits to a FakeDisplay.
    fn apply_edit(&mut self, edit: FakeDisplayEdit) {
        match edit {
            FakeDisplayEdit::SetMode(mode) => {
                // Checks to verify that the mode is one actually supported
                // by this display.
                assert!(self.uuid == mode.uuid);
                assert!(self.modes.contains(&mode));
                self.mode = mode;
            }
            FakeDisplayEdit::SetRotation(rotation) => {
                self.rotation = rotation;
            }
            FakeDisplayEdit::SetOrigin(origin) => {
                self.origin = origin;
            }
            FakeDisplayEdit::SetEnabled(enabled) => {
                self.enabled = enabled;
            }
        }
    }
}

impl Display for FakeDisplay {
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

    type DisplayModeType = FakeDisplayMode;

    fn current_mode(&self) -> &Self::DisplayModeType {
        &self.mode
    }

    fn possible_modes(&self) -> &[Self::DisplayModeType] {
        &self.modes.as_slice()
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct FakeDisplayState {
    displays: BTreeMap<String, FakeDisplay>,
}

static CURRENT_FAKE_DISPLAYS: Lazy<Mutex<BTreeMap<String, FakeDisplay>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

impl FakeDisplayState {
    // Intended for testing, but currently not used.
    #[allow(dead_code)]
    fn set_displays(displays: BTreeMap<String, FakeDisplay>) {
        *CURRENT_FAKE_DISPLAYS.lock().unwrap() = displays;
    }
}

impl DisplayState for FakeDisplayState {
    fn current() -> Result<Self, Error> {
        // The current semantics is that once a display becomes disabled, it
        // will no longer appear in the list of available displays.  So we
        // filter them out before returning the current state.
        let enabled_displays: BTreeMap<String, FakeDisplay> = CURRENT_FAKE_DISPLAYS
            .lock()
            .map_err(|pe| Error::Poisoned(format!("{}", pe)))?
            .clone()
            .into_iter()
            .filter(|(_, display)| display.enabled)
            .collect();
        Ok(Self {
            displays: enabled_displays,
        })
    }

    type DisplayModeType = FakeDisplayMode;

    type DisplayType = FakeDisplay;
    fn get_displays(&self) -> &BTreeMap<String, Self::DisplayType> {
        &self.displays
    }

    type DisplayConfigTransactionType = FakeDisplayConfigTransaction;

    fn configure(&self) -> Result<Self::DisplayConfigTransactionType, Error> {
        FakeDisplayConfigTransaction::new(&self.displays)
    }
}
