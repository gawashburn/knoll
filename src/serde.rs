#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
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

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        use Error::*;
        match self {
            DeRon(e) => Some(e),
            SerRon(e) => Some(e),
            DeJson(e) => Some(e),
            SerJson(e) => Some(e),
            Utf8Conversion(e) => Some(e),
            UnsupportedFormat(_) => None,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            fs => Err(Error::UnsupportedFormat(fs.to_owned())),
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod format_tests {
    use super::*;
    use coverage_helper::test;
    use std::error::Error as StdError;

    #[test]
    fn test_format_from() {
        assert_eq!(Format::from("ron").unwrap(), Format::Ron);
        assert_eq!(Format::from("json").unwrap(), Format::Json);
    }

    #[test]
    fn test_format_from_unsupported() {
        let err = Format::from("xml").unwrap_err();
        assert!(matches!(&err, Error::UnsupportedFormat(fs) if fs == "xml"));
        assert_eq!(format!("{}", err), "Unsupported format: xml");
        assert!(err.source().is_none());
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Helper to abstract over serialization, parameterized by the selected
/// data format.
pub fn serialize<S: Serialize, W: Write>(format: Format, s: &S, writer: W) -> Result<(), Error> {
    match format {
        Format::Ron => {
            let pretty_config = ron::ser::PrettyConfig::new();
            ron::Options::default().to_io_writer_pretty(writer, s, pretty_config)?
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

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod serialization_tests {
    use super::*;
    use coverage_helper::test;
    use serde::{Deserialize, Serialize};
    use std::error::Error as StdError;

    // Simple struct for testing purposes.
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestStruct {
        a: i32,
        b: String,
    }

    #[test]
    fn test_serialize_to_string_and_deserialize_ron() {
        let value = TestStruct {
            a: 1,
            b: "hello".into(),
        };
        let s = serialize_to_string(Format::Ron, &value).unwrap();
        let back: TestStruct = deserialize(Format::Ron, &s).unwrap();
        assert_eq!(value, back);
    }

    #[test]
    fn test_serialize_to_string_and_deserialize_json() {
        let value = TestStruct {
            a: 42,
            b: "world".into(),
        };
        let s = serialize_to_string(Format::Json, &value).unwrap();
        let back: TestStruct = deserialize(Format::Json, &s).unwrap();
        assert_eq!(value, back);
    }

    #[test]
    fn test_deserialize_invalid_ron() {
        let invalid = "(a: )";
        let err = deserialize::<TestStruct>(Format::Ron, invalid).unwrap_err();
        assert!(matches!(&err, Error::DeRon(_)), "Expected DeRon error");
        assert!(err.source().is_some());
        assert!(format!("{}", err).starts_with("RON deserialization error:"));
    }

    #[test]
    fn test_deserialize_invalid_json() {
        let invalid = "{a:}";
        let err = deserialize::<TestStruct>(Format::Json, invalid).unwrap_err();
        assert!(matches!(&err, Error::DeJson(_)), "Expected DeJson error");
        assert!(err.source().is_some());
        assert!(format!("{}", err).starts_with("JSON deserialization error:"));
    }

    #[test]
    fn test_utf8_conversion_error() {
        let invalid = vec![0xff, 0xfe, 0xfd];
        let e = String::from_utf8(invalid).unwrap_err();
        let err: Error = e.into();
        assert!(
            matches!(&err, Error::Utf8Conversion(_)),
            "Expected Utf8Conversion error"
        );
        assert!(err.source().is_some());
        assert!(format!("{}", err).starts_with("Error converting from UTF-8 to string format:"));
    }
}
