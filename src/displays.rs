///! Traits providing an abstract interface for inspecting and modifying the
/// system's display state.
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Formatter;
use std::hash::Hash;

use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_tuple::{Deserialize_tuple, Serialize_tuple};

////////////////////////////////////////////////////////////////////////////////

/// A display rotation.
/// Based upon the options supported in System Settings, and all the
/// data I have available to me, it would seem that macOS only
/// supports cardinal angles.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy, Deserialize_repr, Serialize_repr)]
#[repr(u16)]
pub enum Rotation {
    Zero = 0,
    Ninety = 90,
    OneEighty = 180,
    TwoSeventy = 270,
}

impl Rotation {
    /// Constant containing all the possible `Rotation` values.
    pub const VALUES: [Rotation; 4] = {
        use Rotation::*;
        [Zero, Ninety, OneEighty, TwoSeventy]
    };
}

impl From<Rotation> for f64 {
    fn from(value: Rotation) -> Self {
        (value as i16) as f64
    }
}

impl From<Rotation> for i32 {
    fn from(value: Rotation) -> Self {
        value as i32
    }
}

impl std::fmt::Display for Rotation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", (*self as i32))
    }
}

impl TryFrom<f64> for Rotation {
    type Error = String;
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Rotation::VALUES
            .into_iter()
            .find(|&rotation| {
                let fvalue: f64 = rotation.into();
                value == fvalue
            })
            .ok_or(format!(
                "{} is not currently an allowed Rotation value.",
                value
            ))
    }
}

////////////////////////////////////////////////////////////////////////////////

/// A generic point abstraction.  
/// This could perhaps be better named as we also overload it represent the
/// extents of of a 2D rectangle in space.  However, this is common in many
/// libraries, so it will probably not prove too confusing.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Deserialize_tuple, Serialize_tuple)]
pub struct Point {
    /// The location along the X-axis, or a width.
    pub x: i64,
    /// The location along the Y-axis, or a height.
    pub y: i64,
}

impl std::fmt::Display for Point {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub enum Error {
    /// Reported when attempting to reference a display by an invalid UUID.
    /// The argument is the UUID.
    UnknownUUID(String),
    /// Error reported when a configuration request for a given display is
    /// made a against a DisplayTransaction more than once.
    /// The argument is the UUID.
    DuplicateConfiguration(String),
    /// Reported when a configuration operation is attempted on an
    /// invalid DisplayConfigTransaction.
    InvalidTransactionState,
    /// Reported if there is some underlying locking error.
    /// The argument is the error message.
    Poisoned(String),
    /// A failure arising from interaction with the underlying operating
    /// system.
    /// The argument is the error message.
    Internal(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UnknownUUID(uuid) => {
                write!(
                    f,
                    "Attempted to configure a non-existent display with UUID {}",
                    uuid
                )
            }
            Error::DuplicateConfiguration(uuid) => {
                write!(
                    f,
                    "Attempted to change a setting more than once on display \
                    with UUID {}",
                    uuid
                )
            }
            Error::InvalidTransactionState => {
                write!(
                    f,
                    "While attempting to configure displays the configuration \
                transaction became invalid."
                )
            }
            Error::Poisoned(msg) => {
                write!(f, "Lock poison error: {}", msg)
            }
            Error::Internal(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

////////////////////////////////////////////////////////////////////////////////

/// A representation of one a possible `Display` configuration state.
/// This abstraction is currently leaky as it is not generally
/// valid to pass a `DisplayMode` for a specific `Display` instance
/// when configuring a separate instance.  In some possible future,
/// it might be possible to static prevent confusing `DisplayMode`s in
/// this fashion using something like path dependent types.  In the
/// meantime, this is dynamically checked.
pub trait DisplayMode: Clone + std::fmt::Debug + Serialize {
    /// Return whether the display mode is scaled (2x rather than 1x).
    fn scaled(&self) -> bool;
    /// Return the color depth of the display mode in bits.
    fn color_depth(&self) -> usize;
    /// Return the refresh frequency of the display mode in Hertz.
    /// Some displays may report a frequency of 0.
    fn frequency(&self) -> usize;
    /// Returns the display mode resolution in pixels.  Note this will
    /// be normalized independent of display rotation.  So this will
    /// always correspond to the resolution of the display in landscape
    /// orientation.
    fn extents(&self) -> &Point;

    /// Check whether this display mode matches the given pattern.
    fn match_pattern(&self, pattern: &DisplayModePattern) -> bool {
        pattern.scaled.iter().all(|&s| s == self.scaled())
            && pattern.color_depth.iter().all(|&d| d == self.color_depth())
            && pattern.frequency.iter().all(|&f| f == self.frequency())
            && pattern.extents.iter().all(|p| p == self.extents())
    }
}

/// A `DisplayModePattern` specifies a space of possible `DisplayModes`.
// TODO We could consider extending the constraints allowed by patterns,
//   but it would eliminate some of the symmetry between the input and output
//   formats.
#[derive(Debug, Clone)]
pub struct DisplayModePattern {
    /// Should this pattern match on whether the display mode is scaled?
    pub scaled: Option<bool>,
    /// Should the pattern match on the color depth of the display mode?
    pub color_depth: Option<usize>,
    /// Should this pattern match on the frequency of the display mode?
    pub frequency: Option<usize>,
    /// Should the pattern match on the resolution of the display mode?
    pub extents: Option<Point>,
}

////////////////////////////////////////////////////////////////////////////////

/// A representation of the current state of an attached display.
pub trait Display: std::fmt::Debug {
    /// Obtain the UUID of this display.
    fn uuid(&self) -> &str;

    /// Is this display enabled?  Currently, due to limitations of the APIs
    /// currently used, this will always be true as disabled displays will
    /// not be reported as being attached.
    fn enabled(&self) -> bool;

    /// Where is the upper left corner of this display located?
    fn origin(&self) -> &Point;

    /// What is the current rotation state of the display?
    fn rotation(&self) -> Rotation;

    /// The type of the display mode associated with this Display.
    // TODO Perhaps in the future we could more tightly couple this with
    //   something akin to path dependent types.  For now dynamically check
    //   that DisplayModes are not used with the incorrect display.
    type DisplayModeType: DisplayMode;

    /// Obtain the currently configured. display mode.
    fn current_mode(&self) -> &Self::DisplayModeType;

    /// Obtain all possible display modes for this display.
    fn possible_modes(&self) -> &[Self::DisplayModeType];

    /// Helper to return those display modes for this display that match
    /// the provided pattern.
    fn matching_modes(&self, pattern: &DisplayModePattern) -> Vec<Self::DisplayModeType> {
        self.possible_modes()
            .iter()
            .flat_map(|m| {
                if m.match_pattern(pattern) {
                    Some(m.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

////////////////////////////////////////////////////////////////////////////////

/// An abstraction for representing a configuration "transaction" that will
/// update the overall state of the attached displays.
///
/// It might have been more elegant from API perspective to allow a
/// `DisplayConfigTransaction` allow returning a map of "ConfigurableDisplays"
/// analogous to `DisplayState` instead of having all the operations take a
/// UUID.  However, if we also want to enforce that the transaction is used
/// effectively linearly and consumed either by committing or causing it to
/// be dropped, at best it requires non-trivial uses of references and lifetime
/// parameters.  
pub trait DisplayConfigTransaction {
    /// The type of display modes that this transaction will accept.
    type DisplayModeType: DisplayMode;

    /// Set the display mode the given display.
    /// Will return an error if there is no display with the given UUID.
    fn set_mode(&mut self, uuid: &str, mode: &Self::DisplayModeType) -> Result<(), Error>;

    /// Set the rotation of the given display.
    /// Will return an error if there is no display with the given UUID.
    fn set_rotation(&mut self, uuid: &str, rotation: Rotation) -> Result<(), Error>;

    /// Set the rotation of the given display.
    /// Will return an error if there is no display with the given UUID.
    fn set_origin(&mut self, uuid: &str, point: &Point) -> Result<(), Error>;

    /// Set the enablement state of the given display.  Given current API
    /// limitations, once a display is disabled, and the configuration
    /// completes, it will no longer register as attached.
    /// Will return an error if there is no display with the given UUID.
    fn set_enabled(&mut self, uuid: &str, enabled: bool) -> Result<(), Error>;

    /// Attempt to apply the requested configuration changes and close out
    /// the transaction.
    fn commit(self) -> Result<(), Error>;
}

////////////////////////////////////////////////////////////////////////////////

/// An abstract representation of the currently attached displays.
pub trait DisplayState: Sized {
    /// Obtain the current display state.
    fn current() -> Result<Self, Error>;

    /// The type of display modes used by displays.
    type DisplayModeType: DisplayMode;
    /// The type of displays.  It must be the case that uses the same
    /// type for display modes as the `DisplayState` does.
    type DisplayType: Display<DisplayModeType = Self::DisplayModeType>;
    /// The type of a configuration transaction.  It must be the case that
    /// uses the same type for display modes as the `DisplayState` does.
    type DisplayConfigTransactionType: DisplayConfigTransaction<
        DisplayModeType = Self::DisplayModeType,
    >;

    /// Obtain a map of UUIDs to `Display`s
    fn get_displays(&self) -> &BTreeMap<String, Self::DisplayType>;

    /// Obtain a configuration transaction that can be used to modify the
    /// state of attached displays.  Note that any changes applied when
    /// `complete()` is called on the resulting configuration transaction,
    /// will not be reflected this `DisplayState`.  To observe the
    /// changes it will be necessary to obtain the new state with
    /// `current()`.
    fn configure(&self) -> Result<Self::DisplayConfigTransactionType, Error>;
}
