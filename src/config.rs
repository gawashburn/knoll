#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use crate::displays::Point;
use crate::displays::Rotation;
///! Data structures used for representing the current state of the attached
/// displays as well as requesting changes to that configuration.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::PartialEq;

////////////////////////////////////////////////////////////////////////////////

/// Helper to serialize Option values as just the value itself.  It does not
/// need to handle the case of None, as it is intended to be used with the
/// serde option `skip_serializing_if = "Option::is_none"`.
fn serialize_opt<S, T>(opt: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    assert!(opt.is_some(), "serialize_opt should never receive None");
    opt.as_ref().unwrap().serialize(serializer)
}

/// Helper to deserialize values to Option by wrapping them in Some.
fn deserialize_opt<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Some(T::deserialize(deserializer)?))
}

////////////////////////////////////////////////////////////////////////////////

/// Some basic sanity checking for `serialize_opt`.
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_serialize_opt() {
    {
        let mut buffer = String::new();
        let ron_pretty = ron::ser::PrettyConfig::default();
        let mut ron_ser = ron::ser::Serializer::new(&mut buffer, Some(ron_pretty))
            .expect("Constructing serializer should not fail.");

        serialize_opt(&Some(0), &mut ron_ser).expect("Serialization should not fail.");
        assert_eq!(buffer, "0");
    }

    {
        let mut buffer = vec![];
        {
            let mut json_ser = serde_json::ser::Serializer::new(&mut buffer);
            serialize_opt(&Some(0), &mut json_ser).expect("Serialization should not fail.");
        }
        assert_eq!(
            String::from_utf8(buffer.clone()).expect("String should be valid UTF-8."),
            "0"
        );
    }
}

/// Some basic sanity checking for `deserialize_opt`.
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_deserialize_opt() {
    let mut json_de = serde_json::de::Deserializer::from_str("0");
    let result: Option<u64> =
        deserialize_opt(&mut json_de).expect("Deserialization should not fail");
    assert_eq!(result, Some(0));

    let mut ron_de =
        ron::de::Deserializer::from_str("0").expect("Constructing deserializer should not fail.");
    let result: Option<u64> =
        deserialize_opt(&mut ron_de).expect("Deserialization should not fail");
    assert_eq!(result, Some(0));
}

////////////////////////////////////////////////////////////////////////////////

/// A Config describes how to configure an individual display.
#[derive(Debug, PartialEq, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub uuid: String,
    // TODO Is there a way to avoid repeating the same attributes?
    // It might be possible with macros, but so far I have not found
    // any notion of an defining an attribute alias.
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_opt",
        deserialize_with = "deserialize_opt",
        default
    )]
    pub mirror_of: Option<String>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_opt",
        deserialize_with = "deserialize_opt",
        default
    )]
    pub enabled: Option<bool>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_opt",
        deserialize_with = "deserialize_opt",
        default
    )]
    pub origin: Option<Point>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_opt",
        deserialize_with = "deserialize_opt",
        default
    )]
    pub extents: Option<Point>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_opt",
        deserialize_with = "deserialize_opt",
        default
    )]
    pub scaled: Option<bool>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_opt",
        deserialize_with = "deserialize_opt",
        default
    )]
    pub frequency: Option<usize>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_opt",
        deserialize_with = "deserialize_opt",
        default
    )]
    pub color_depth: Option<usize>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_opt",
        deserialize_with = "deserialize_opt",
        default
    )]
    pub rotation: Option<Rotation>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_opt",
        deserialize_with = "deserialize_opt",
        default
    )]
    pub brightness: Option<f32>,
}

/// A ConfigGroup describes how to configure a group attached of displays.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConfigGroup {
    /// Order is irrelevant, but it would require some additional effort
    /// to implement Hash for the HashSet in Config.
    pub configs: Vec<Config>,
}

/// ConfigGroups is simply a collection of ConfigGroups for different
/// possible system configurations
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConfigGroups {
    /// Order is irrelevant, but it would require some additional effort
    /// to implement Hash for the HashSet in Config.
    pub groups: Vec<ConfigGroup>,
}

////////////////////////////////////////////////////////////////////////////////

/// Sanity check configuration serialization.
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_serialization() {
    let c1 = Config::default();
    let c2 = Config {
        uuid: "ab3456def".to_owned(),
        mirror_of: None,
        enabled: Some(true),
        origin: Some(Point { x: 1, y: 2 }),
        extents: Some(Point { x: 3, y: 6 }),
        scaled: Some(true),
        frequency: Some(60),
        color_depth: Some(8),
        rotation: Some(Rotation::Ninety),
        brightness: Some(1.0),
    };

    let c1_json_str =
        serde_json::ser::to_string_pretty(&c1).expect("Serialization should not fail");
    assert_eq!(
        c1_json_str,
        r#"{
  "uuid": ""
}"#
    );

    let c2_json_str =
        serde_json::ser::to_string_pretty(&c2).expect("Serialization should not fail");
    assert_eq!(
        c2_json_str,
        r#"{
  "uuid": "ab3456def",
  "enabled": true,
  "origin": [
    1,
    2
  ],
  "extents": [
    3,
    6
  ],
  "scaled": true,
  "frequency": 60,
  "color_depth": 8,
  "rotation": 90,
  "brightness": 1.0
}"#
    );

    let ron_pretty = ron::ser::PrettyConfig::new();

    let c1_ron_str =
        ron::ser::to_string_pretty(&c1, ron_pretty.clone()).expect("Serialization should not fail");
    assert_eq!(
        c1_ron_str,
        r#"(
    uuid: "",
)"#
    );

    let c2_ron_str =
        ron::ser::to_string_pretty(&c2, ron_pretty.clone()).expect("Serialization should not fail");
    assert_eq!(
        c2_ron_str,
        r#"(
    uuid: "ab3456def",
    enabled: true,
    origin: (1, 2),
    extents: (3, 6),
    scaled: true,
    frequency: 60,
    color_depth: 8,
    rotation: 90,
    brightness: 1.0,
)"#
    );

    let cg1 = ConfigGroup {
        configs: vec![c1.clone(), c2.clone()],
    };

    let cg2 = ConfigGroup { configs: vec![c1] };

    let cg1_json_str =
        serde_json::ser::to_string_pretty(&cg1).expect("Serialization should not fail");
    assert_eq!(
        cg1_json_str,
        r#"[
  {
    "uuid": ""
  },
  {
    "uuid": "ab3456def",
    "enabled": true,
    "origin": [
      1,
      2
    ],
    "extents": [
      3,
      6
    ],
    "scaled": true,
    "frequency": 60,
    "color_depth": 8,
    "rotation": 90,
    "brightness": 1.0
  }
]"#
    );

    let cg1_ron_str = ron::ser::to_string_pretty(&cg1, ron_pretty.clone())
        .expect("Serialization should not fail");
    assert_eq!(
        cg1_ron_str,
        r#"[
    (
        uuid: "",
    ),
    (
        uuid: "ab3456def",
        enabled: true,
        origin: (1, 2),
        extents: (3, 6),
        scaled: true,
        frequency: 60,
        color_depth: 8,
        rotation: 90,
        brightness: 1.0,
    ),
]"#
    );

    let cg2_json_str =
        serde_json::ser::to_string_pretty(&cg2).expect("Serialization should not fail");
    assert_eq!(
        cg2_json_str,
        r#"[
  {
    "uuid": ""
  }
]"#
    );
    let cg2_ron_str = ron::ser::to_string_pretty(&cg2, ron_pretty.clone())
        .expect("Serialization should not fail");
    assert_eq!(
        cg2_ron_str,
        r#"[
    (
        uuid: "",
    ),
]"#
    );

    let cgs1 = ConfigGroups {
        groups: vec![cg1.clone(), cg2.clone()],
    };
    let cgs2 = ConfigGroups { groups: vec![cg2] };

    let cgs1_json_str =
        serde_json::ser::to_string_pretty(&cgs1).expect("Serialization should not fail");
    assert_eq!(
        cgs1_json_str,
        r#"[
  [
    {
      "uuid": ""
    },
    {
      "uuid": "ab3456def",
      "enabled": true,
      "origin": [
        1,
        2
      ],
      "extents": [
        3,
        6
      ],
      "scaled": true,
      "frequency": 60,
      "color_depth": 8,
      "rotation": 90,
      "brightness": 1.0
    }
  ],
  [
    {
      "uuid": ""
    }
  ]
]"#
    );
    let cgs1_ron_str = ron::ser::to_string_pretty(&cgs1, ron_pretty.clone())
        .expect("Serialization should not fail");
    assert_eq!(
        cgs1_ron_str,
        r#"[
    [
        (
            uuid: "",
        ),
        (
            uuid: "ab3456def",
            enabled: true,
            origin: (1, 2),
            extents: (3, 6),
            scaled: true,
            frequency: 60,
            color_depth: 8,
            rotation: 90,
            brightness: 1.0,
        ),
    ],
    [
        (
            uuid: "",
        ),
    ],
]"#
    );

    let cgs2_json_str =
        serde_json::ser::to_string_pretty(&cgs2).expect("Serialization should not fail");
    assert_eq!(
        cgs2_json_str,
        r#"[
  [
    {
      "uuid": ""
    }
  ]
]"#
    );
    let cgs2_ron_str = ron::ser::to_string_pretty(&cgs2, ron_pretty.clone())
        .expect("Serialization should not fail");
    assert_eq!(
        cgs2_ron_str,
        r#"[
    [
        (
            uuid: "",
        ),
    ],
]"#
    );
}

/// Sanity check configuration deserialization.
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_deserialization() {
    match serde_json::de::from_str::<'static, Config>("{}") {
        Err(_) => { /* Failed as expected, so no-op */ }
        _ => panic!("Deserialization should have failed"),
    };

    match ron::de::from_str::<'static, Config>("()") {
        Err(_) => { /* Failed as expected, so no-op */ }
        _ => panic!("Deserialization should have failed"),
    };

    let c: Config = serde_json::de::from_str("{\"uuid\":\"abcdef1234\"}")
        .expect("Deserialization should not fail");
    assert_eq!(
        c,
        Config {
            uuid: "abcdef1234".to_owned(),
            mirror_of: None,
            enabled: None,
            origin: None,
            extents: None,
            scaled: None,
            frequency: None,
            color_depth: None,
            rotation: None,
            brightness: None,
        }
    );

    let c: Config =
        serde_json::de::from_str("{\"uuid\":\"abcdef1234\",\"enabled\": false, \"origin\":[1,2]}")
            .expect("Deserialization should not fail");
    assert_eq!(
        c,
        Config {
            uuid: "abcdef1234".to_owned(),
            mirror_of: None,
            enabled: Some(false),
            origin: Some(Point { x: 1, y: 2 }),
            extents: None,
            scaled: None,
            frequency: None,
            color_depth: None,
            rotation: None,
            brightness: None,
        }
    );

    let c: Config = serde_json::de::from_str("[\"abcdef123\", \"fedcba456\", true]")
        .expect("Deserialization should not fail");
    assert_eq!(
        c,
        Config {
            uuid: "abcdef123".to_owned(),
            mirror_of: Some("fedcba456".to_owned()),
            enabled: Some(true),
            origin: None,
            extents: None,
            scaled: None,
            frequency: None,
            color_depth: None,
            rotation: None,
            brightness: None,
        }
    );

    let c: Config =
        ron::de::from_str("(uuid: \"abcdef1234\")").expect("Deserialization should not fail");
    assert_eq!(
        c,
        Config {
            uuid: "abcdef1234".to_owned(),
            mirror_of: None,
            enabled: None,
            origin: None,
            extents: None,
            scaled: None,
            frequency: None,
            color_depth: None,
            rotation: None,
            brightness: None,
        }
    );

    let c: Config = ron::de::from_str("(uuid: \"abcdef1234\", enabled: false)")
        .expect("Deserialization should not fail");
    assert_eq!(
        c,
        Config {
            uuid: "abcdef1234".to_owned(),
            mirror_of: None,
            enabled: Some(false),
            origin: None,
            extents: None,
            scaled: None,
            frequency: None,
            color_depth: None,
            rotation: None,
            brightness: None,
        }
    );

    let c: Config = ron::de::from_str("(uuid: \"abcdef1234\", origin:(1,2))")
        .expect("Deserialization should not fail");
    assert_eq!(
        c,
        Config {
            uuid: "abcdef1234".to_owned(),
            mirror_of: None,
            enabled: None,
            origin: Some(Point { x: 1, y: 2 }),
            extents: None,
            scaled: None,
            frequency: None,
            color_depth: None,
            rotation: None,
            brightness: None,
        }
    );

    let c: Config =
        ron::de::from_str("(uuid: \"abcdef1234\", enabled: false, origin:(0,1), rotation:180)")
            .expect("Deserialization should not fail");
    assert_eq!(
        c,
        Config {
            uuid: "abcdef1234".to_owned(),
            mirror_of: None,
            enabled: Some(false),
            origin: Some(Point { x: 0, y: 1 }),
            extents: None,
            scaled: None,
            frequency: None,
            color_depth: None,
            rotation: Some(Rotation::OneEighty),
            brightness: None,
        }
    );

    match ron::de::from_str::<'static, Config>("(uuid: \"abcdef1234\", rotation:45)") {
        Err(e) => {
            // Validate expected error message.
            assert!(
                format!("{}", e).contains("invalid value: 45, expected one of: 0, 90, 180, 270")
            );
        }
        Ok(_) => panic!("Deserialization should have failed."),
    }

    let c: Config =
        ron::de::from_str("(uuid: \"abcdef1234\", mirror_of: \"5678fedcba\", enabled: true)")
            .expect("Deserialization should not fail");
    assert_eq!(
        c,
        Config {
            uuid: "abcdef1234".to_owned(),
            mirror_of: Some("5678fedcba".to_owned()),
            enabled: Some(true),
            origin: None,
            extents: None,
            scaled: None,
            frequency: None,
            color_depth: None,
            rotation: None,
            brightness: None,
        }
    );

    let c: Config =
        serde_json::de::from_str("{\"uuid\":\"abcdef1234\",\"mirror_of\":\"5678fedcba\"}")
            .expect("Deserialization should not fail");
    assert_eq!(
        c,
        Config {
            uuid: "abcdef1234".to_owned(),
            mirror_of: Some("5678fedcba".to_owned()),
            enabled: None,
            origin: None,
            extents: None,
            scaled: None,
            frequency: None,
            color_depth: None,
            rotation: None,
            brightness: None,
        }
    );

    let cg: ConfigGroup = serde_json::de::from_str("[{\"uuid\":\"abcdef1234\"}]")
        .expect("Deserialization should not fail");
    assert_eq!(
        cg,
        ConfigGroup {
            configs: vec![Config {
                uuid: "abcdef1234".to_owned(),
                mirror_of: None,
                enabled: None,
                origin: None,
                extents: None,
                scaled: None,
                frequency: None,
                color_depth: None,
                rotation: None,
                brightness: None,
            }]
        }
    );

    let cg: ConfigGroup = ron::de::from_str("[(uuid: \"abcdef1234\", origin:(1,2))]")
        .expect("Deserialization should not fail");
    assert_eq!(
        cg,
        ConfigGroup {
            configs: vec![Config {
                uuid: "abcdef1234".to_owned(),
                mirror_of: None,
                enabled: None,
                origin: Some(Point { x: 1, y: 2 }),
                extents: None,
                scaled: None,
                frequency: None,
                color_depth: None,
                rotation: None,
                brightness: None,
            }]
        }
    );

    let cgs: ConfigGroups = ron::de::from_str("[[(uuid: \"abcdef1234\", origin:(1,2))]]")
        .expect("Deserialization should not fail");
    assert_eq!(
        cgs,
        ConfigGroups {
            groups: vec![ConfigGroup {
                configs: vec![Config {
                    uuid: "abcdef1234".to_owned(),
                    mirror_of: None,
                    enabled: None,
                    origin: Some(Point { x: 1, y: 2 }),
                    extents: None,
                    scaled: None,
                    frequency: None,
                    color_depth: None,
                    rotation: None,
                    brightness: None,
                }]
            }]
        }
    );
}
