use serde::{Deserialize, Serialize};
use std::fmt::Formatter;
use std::io::Write;

////////////////////////////////////////////////////////////////////////////////

/// Wrapper for the different serialization and deserialization failure modes.
#[derive(Debug)]
pub enum Error {
    DeRon(ron::error::SpannedError),
    SerRon(ron::error::Error),
    DeJson(serde_json::Error),
    SerJson(serde_json::Error),
    Utf8Conversion(std::string::FromUtf8Error),
    UnsupportedFormat(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use Error::*;

        match self {
            DeRon(e) => {
                write!(f, "RON deserialization error: {}", e)
            }
            SerRon(e) => {
                write!(f, "RON serialization error: {}", e)
            }
            DeJson(e) => {
                write!(f, "JSON deserialization error: {}", e)
            }
            SerJson(e) => {
                write!(f, "JSON serialization error: {}", e)
            }
            Utf8Conversion(e) => {
                write!(f, "Error converting from UTF-8 to string format: {}", e)
            }
            UnsupportedFormat(fs) => {
                write!(f, "Unsupported format: {}", fs)
            }
        }
    }
}

impl From<ron::error::SpannedError> for Error {
    fn from(e: ron::error::SpannedError) -> Self {
        Error::DeRon(e)
    }
}

impl From<ron::error::Error> for Error {
    fn from(e: ron::error::Error) -> Self {
        Error::SerRon(e)
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(e: std::string::FromUtf8Error) -> Self {
        Error::Utf8Conversion(e)
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Tag class to specify the kind of (de)serialization to be used.
#[derive(Debug, Clone, Copy)]
pub enum Format {
    Ron,
    Json,
}

impl Format {
    /// Construct a `Format` from a string.
    pub fn from(str: &str) -> Result<Self, Error> {
        match str {
            "ron" => Ok(Format::Ron),
            "json" => Ok(Format::Json),
            fs => Err(Error::UnsupportedFormat(String::from(fs))),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Helper to abstract over serialization, parameterized by the selected
/// data format.
pub fn serialize<S: Serialize, W: Write>(format: Format, s: &S, writer: W) -> Result<(), Error> {
    match format {
        Format::Ron => {
            let pretty_config = ron::ser::PrettyConfig::new();
            ron::ser::to_writer_pretty(writer, s, pretty_config)?
        }
        Format::Json => serde_json::ser::to_writer_pretty(writer, s).map_err(Error::SerJson)?,
    }
    Ok(())
}

/// Helper to abstract over serialization to a String, parameterized by the selected
/// data format.
pub fn serialize_to_string<S: Serialize>(format: Format, s: &S) -> Result<String, Error> {
    let mut vec: Vec<u8> = Vec::new();
    serialize(format, s, &mut vec)?;
    let str = String::from_utf8(vec)?;
    Ok(str)
}

/// Helper to abstract over deserialization, parameterized by the selected
/// data format.
pub fn deserialize<'a, D: Deserialize<'a>>(format: Format, str: &'a str) -> Result<D, Error> {
    Ok(match format {
        Format::Ron => ron::de::from_str(str)?,
        Format::Json => serde_json::from_str(str).map_err(Error::DeJson)?,
    })
}
