///! Data structures used for representing the current state of the attached
/// displays as well as requesting changes to that configuration.
use crate::displays::Point;
use crate::displays::Rotation;
use std::collections::HashSet;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::{Eq, PartialEq};

////////////////////////////////////////////////////////////////////////////////

/// Helper to serialize Option values as just the value itself.
fn serialize_opt<S, T>(opt: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
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

/// A Config describes how to configure an individual display.
#[derive(Debug, PartialEq, Eq, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub uuid: String,
    #[serde(skip_serializing_if = "HashSet::is_empty", default)]
    pub mirrors: HashSet<String>,
    // TODO Is there a way to avoid repeating the same attributes?
    // It might be possible with macros, but so far I have not found
    // any notion of an defining an attribute alias.
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
}

/// A ConfigGroup describes how to configure a group attached of displays.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConfigGroup {
    /// Order is irrelevant, but it would require some additional effort
    /// to implement Hash for the HashSet in Config.
    pub configs: Vec<Config>,
}

/// ConfigGroups is simply a collection of ConfigGroups for different
/// possible system configurations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConfigGroups {
    /// Order is irrelevant, but it would require some additional effort
    /// to implement Hash for the HashSet in Config.
    pub groups: Vec<ConfigGroup>,
}

////////////////////////////////////////////////////////////////////////////////

#[test]
fn test_serialization() {
    let c1 = Config::default();
    let c2 = Config {
        uuid: String::from("ab3456def"),
        mirrors: HashSet::from([String::from("def456")]),
        enabled: Some(true),
        origin: Some(Point { x: 1, y: 2 }),
        extents: Some(Point { x: 3, y: 6 }),
        scaled: Some(true),
        frequency: Some(60),
        color_depth: Some(8),
        rotation: Some(Rotation::Ninety),
    };

    let c1str = serde_json::ser::to_string_pretty(&c1).unwrap();
    println!("c1: {}", c1str);
    let c2str = serde_json::ser::to_string_pretty(&c2).unwrap();
    println!("c2: {}", c2str);
    let ron_pretty = ron::ser::PrettyConfig::new().struct_names(false);
    let c3str = ron::ser::to_string_pretty(&c2, ron_pretty).unwrap();
    println!("c2r: {}", c3str);

    let cg1 = ConfigGroup {
        configs: vec![c1.clone(), c2.clone()],
    };

    let cg2 = ConfigGroup { configs: vec![c1] };

    let cg1str = serde_json::ser::to_string_pretty(&cg1).unwrap();
    println!("cg1: {}", cg1str);
    let ron_pretty = ron::ser::PrettyConfig::new().struct_names(false);
    let cg2str = ron::ser::to_string_pretty(&cg1, ron_pretty).unwrap();
    println!("cg1r: {}", cg2str);
    let cg3str = serde_json::ser::to_string_pretty(&cg2).unwrap();
    println!("cg2: {}", cg3str);
    let ron_pretty = ron::ser::PrettyConfig::new().struct_names(false);
    let cg4str = ron::ser::to_string_pretty(&cg2, ron_pretty).unwrap();
    println!("cg2r: {}", cg4str);

    let cgs1 = ConfigGroups {
        groups: vec![cg1.clone()],
    };
    let cgs2 = ConfigGroups { groups: vec![cg2] };

    let cgs1str = serde_json::ser::to_string_pretty(&cgs1).unwrap();
    println!("cgs1: {}", cgs1str);
    let ron_pretty = ron::ser::PrettyConfig::new().struct_names(false);
    let cgs2str = ron::ser::to_string_pretty(&cgs1, ron_pretty).unwrap();
    println!("cg1r: {}", cgs2str);

    let cgs3str = serde_json::ser::to_string_pretty(&cgs2).unwrap();
    println!("cgs2: {}", cgs3str);
    let ron_pretty = ron::ser::PrettyConfig::new().struct_names(false);
    let cgs4str = ron::ser::to_string_pretty(&cgs2, ron_pretty).unwrap();
    println!("cg2r: {}", cgs4str);
}

#[test]
fn test_deserialization() {
    let c3: Config = serde_json::de::from_str("{\"uuid\":\"abcdef1234\"}").unwrap();
    println!("c3: {:?}", c3);
    let c4: Config =
        serde_json::de::from_str("{\"uuid\":\"abcdef1234\",\"enabled\": false, \"origin\":[1,2]}")
            .unwrap();
    println!("c4: {:?}", c4);
    let c5: Config = serde_json::de::from_str("[\"abcdef123\", [], true]").unwrap();
    println!("c5: {:?}", c5);
    let c6: Config = ron::de::from_str("(uuid: \"abcdef1234\")").unwrap();
    println!("c6: {:?}", c6);
    let c7: Config = ron::de::from_str("(uuid: \"abcdef1234\", enabled: false)").unwrap();
    println!("c7: {:?}", c7);
    let c8: Config = ron::de::from_str("(uuid: \"abcdef1234\", origin:(1,2))").unwrap();
    println!("c8: {:?}", c8);
    let c9: Config =
        ron::de::from_str("(uuid: \"abcdef1234\", enabled: false, origin:(0,1), rotation:180)")
            .unwrap();
    println!("c9: {:?}", c9);

    let cg1: ConfigGroup = ron::de::from_str("[(uuid: \"abcdef1234\", origin:(1,2))]").unwrap();
    println!("cg1: {:?}", cg1);
    let cg2: ConfigGroup = serde_json::de::from_str("[{\"uuid\":\"abcdef1234\"}]").unwrap();
    println!("cg2: {:?}", cg2);
}
