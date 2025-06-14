#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! This module provides a data structure that is allows for more efficient
//! access to a configuration group that has been validated as being
//! semantically consistent.  That is, it doesn't have duplicate displays
//! and is non-empty.
use std::collections::hash_map::Entry;
use std::collections::{BTreeSet, HashMap, HashSet};

use std::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use std::fmt::Formatter;
use std::hash::{Hash, Hasher};

use crate::config::{Config, ConfigGroup, ConfigGroups};

////////////////////////////////////////////////////////////////////////////////

/// The possible errors that can ar
#[derive(Debug)]
pub enum Error {
    /// Reported when a configuration group contains a display with the same
    /// UUID multiple times.  The argument is a set of the UUIDs that
    /// appear multiple times.
    DuplicateDisplays(HashSet<String>),
    /// Reported with a there are multiple configuration groups that contain
    /// the exact same set of displays.
    DuplicateGroups(HashSet<ValidConfigGroup>),
    /// Reported when a configuration group contains no displays.
    EmptyGroup,
    /// Reported when a display with mirror_of set also has other display options set,
    /// which are not compatible with mirroring.
    InvalidMirrorConfig(String),
    /// Reported when a display is mirroring another display that itself is mirroring.
    MirrorOfMirror(String, String),
    /// Reported when a brightness value is outside the allowed range [0.0,1.0].
    InvalidBrightness(String, f32),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::DuplicateDisplays(uuids) => {
                write!(
                    f,
                    "A configuration group contains a configuration for \
                the same display more than once: {}",
                    uuids.iter().cloned().collect::<Vec<String>>().join(", ")
                )
            }
            Error::DuplicateGroups(configs) => {
                // Collect up all the groups of uuids that are duplicated.
                let dups = configs
                    .iter()
                    .map(|vc| {
                        let uuids = vc.uuids.iter().cloned().collect::<Vec<String>>().join(", ");
                        let mut group = "[".to_owned();
                        group.push_str(uuids.as_str());
                        group.push(']');
                        group
                    })
                    .collect::<Vec<String>>()
                    .join(" ");

                write!(
                    f,
                    "There are multiple configuration groups with the same \
                    set of displays: {}",
                    dups
                )
            }
            Error::EmptyGroup => write!(f, "A configuration group is empty."),
            Error::InvalidMirrorConfig(uuid) => write!(
                f,
                "A display with UUID {} has the mirror_of option set but also \
                has other display options set, which are not compatible with \
                mirroring.",
                uuid
            ),
            Error::MirrorOfMirror(uuid, target_uuid) => write!(
                f,
                "A display with UUID {} is configured to mirror the display \
                with UUID {}, however that display is itself already mirroring \
                another display.",
                uuid, target_uuid
            ),
            Error::InvalidBrightness(uuid, value) => write!(
                f,
                "Display with UUID {} has invalid brightness {}. Brightness must be between 0.0 and 1.0.",
                uuid, value
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct ValidConfigGroup {
    pub uuids: BTreeSet<String>,
    pub configs: HashMap<String, Config>,
}

impl Hash for ValidConfigGroup {
    /// For the purposes of hashing, we hash the individual UUIDs.
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uuids.iter().for_each(|uuid| uuid.hash(state));
    }
}

impl PartialEq for ValidConfigGroup {
    /// Equality is defined by the configuration group having the same set
    /// of UUIDS
    fn eq(&self, other: &Self) -> bool {
        self.uuids == other.uuids
    }
}

impl Eq for ValidConfigGroup {}

impl PartialOrd for ValidConfigGroup {
    /// Ordering is by reverse inclusion.  We consider sets that contain
    /// more elements, or are more "precise", to be "smaller".  For
    /// incomparable configurations, the ordering is based upon size.
    ///
    /// Despite this being ostensibly a "partial ordering", because
    /// Rust's sort only uses `partial_cmp` rather than `cmp`, this has
    /// been made a total order.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        use Ordering::*;
        Some(
            match (
                self.uuids.is_superset(&other.uuids),
                other.uuids.is_superset(&self.uuids),
            ) {
                (true, true) => Equal,
                (true, false) => Less,
                (false, true) => Greater,

                _ => {
                    // There cannot be a true total ordering on configuration
                    // groups, but this definition should be sufficient for
                    // sorting configurations by precision.  However, given
                    // that two incomparable configuration groups of the same
                    // length are effectively treated as equal, if there are
                    // duplicates, there is no guarantee the truly identical
                    // configuration groups will be clustered together.
                    other.uuids.len().cmp(&self.uuids.len())
                }
            },
        )
    }
}

impl Ord for ValidConfigGroup {
    // Disable coverage as this function should never be called, it just
    // exists to satisfy the Ord trait.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn cmp(&self, _other: &Self) -> Ordering {
        panic!("Ord is required sort ValidConfigGroup can be sorted, but isn't actually used.")
    }
}

impl ValidConfigGroup {
    /// Helper to convert a configuration group into a map.  This enforces
    /// that no config in the group has the same UUID and that the group
    /// is non-empty.
    pub fn from(cg: ConfigGroup) -> Result<Self, Error> {
        let mut duplicates = HashSet::new();
        let mut configs = HashMap::new();

        for config in cg.configs {
            let uuid = &config.uuid;

            // Check if mirror_of is set but other options have been set.
            if let Some(_) = &config.mirror_of {
                // When mirroring, only the uuid and mirror_of should have
                // values.
                if config.origin.is_some()
                    || config.extents.is_some()
                    || config.scaled.is_some()
                    || config.frequency.is_some()
                    || config.color_depth.is_some()
                    || config.rotation.is_some()
                    // TODO would it make sense to allow brightness
                    // to be configured distinctly for a mirror?
                    || config.brightness.is_some()
                {
                    return Err(Error::InvalidMirrorConfig(uuid.clone()));
                }
            }

            // Check that the brightness range is valid.
            if let Some(brightness) = config.brightness {
                if brightness < 0.0 || brightness > 1.0 {
                    return Err(Error::InvalidBrightness(uuid.clone(), brightness));
                }
            }

            if let Entry::Vacant(e) = configs.entry(uuid.clone()) {
                e.insert(config);
            } else {
                duplicates.insert(uuid.clone());
            }
        }

        // If there are any duplicate displays report the error.
        if !duplicates.is_empty() {
            Err(Error::DuplicateDisplays(duplicates))
            // A group must have at least one Config.
        } else if configs.is_empty() {
            Err(Error::EmptyGroup)
        } else {
            // Check that no display mirrors a display that is itself mirroring.
            for (uuid, config) in &configs {
                if let Some(target_uuid) = &config.mirror_of {
                    if let Some(target_config) = configs.get(target_uuid) {
                        if target_config.mirror_of.is_some() {
                            return Err(Error::MirrorOfMirror(uuid.clone(), target_uuid.clone()));
                        }
                    }
                }
            }

            Ok(ValidConfigGroup {
                uuids: configs.keys().cloned().collect(),
                configs,
            })
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Check that `ValidConfigGroup::from` correctly reports an error for an
/// empty group.
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_valid_config_from_empty() {
    let result = ValidConfigGroup::from(ConfigGroup { configs: vec![] });
    assert!(
        matches!(result, Err(Error::EmptyGroup)),
        "Failed to detect empty configuration."
    );
}

/// Check that `ValidConfigGroup::from` correctly reports an error for
/// duplicate configurations.
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_valid_config_from_duplicate() {
    let result = ValidConfigGroup::from(ConfigGroup {
        configs: vec![
            Config {
                uuid: "abcdef1234".to_owned(),
                enabled: Some(false),
                ..Config::default()
            },
            Config {
                uuid: "abcdef1234".to_owned(),
                enabled: Some(false),
                ..Config::default()
            },
        ],
    });
    assert!(
        matches!(
        result,
        Err(Error::DuplicateDisplays(uuids)) if uuids.len() == 1 && uuids.contains("abcdef1234")),
        "Failed to detect duplicate configuration."
    );

    let result = ValidConfigGroup::from(ConfigGroup {
        configs: vec![
            Config {
                uuid: "abcdef1234".to_owned(),
                enabled: Some(false),
                ..Config::default()
            },
            Config {
                uuid: "abcdef1234".to_owned(),
                enabled: Some(false),
                ..Config::default()
            },
            Config {
                uuid: "foobarbaz".to_owned(),
                enabled: Some(false),
                ..Config::default()
            },
            Config {
                uuid: "foobarbaz".to_owned(),
                enabled: Some(false),
                ..Config::default()
            },
        ],
    });
    assert!(
        matches!(
            result,
            Err(Error::DuplicateDisplays(uuids)) if uuids.len() == 2 && uuids.contains("abcdef1234") && uuids.contains("foobarbaz")
        ),
        "Failed to detect duplicate configuration."
    );
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod valid_config_group_tests {
    use super::*;
    use crate::displays::Point;

    /// Test that `ValidConfigGroup::from` correctly reports an error for
    /// mirror_of with incompatible display options.
    #[test]
    fn test_valid_config_from_invalid_mirror() {
        let result = ValidConfigGroup::from(ConfigGroup {
            configs: vec![Config {
                uuid: "abcdef1234".to_owned(),
                mirror_of: Some("5678defghi".to_owned()),
                enabled: Some(true), // enabled is allowed with mirror_of
                origin: Some(Point { x: 0, y: 0 }), // origin is not allowed with mirror_of
                ..Config::default()
            }],
        });
        assert!(
            matches!(
                result,
                Err(Error::InvalidMirrorConfig(uuid)) if uuid == "abcdef1234"
            ),
            "Failed to detect invalid mirror configuration."
        );
    }

    /// Test that `ValidConfigGroup::from` correctly reports an error when
    /// a display mirrors a display that itself is mirroring.
    #[test]
    fn test_valid_config_from_mirror_of_mirror() {
        let cg = ConfigGroup {
            configs: vec![
                Config {
                    uuid: "A".to_owned(),
                    mirror_of: Some("B".to_owned()),
                    enabled: Some(true),
                    ..Config::default()
                },
                Config {
                    uuid: "B".to_owned(),
                    mirror_of: Some("C".to_owned()),
                    enabled: Some(true),
                    ..Config::default()
                },
                Config {
                    uuid: "C".to_owned(),
                    enabled: Some(true),
                    ..Config::default()
                },
            ],
        };

        let result = ValidConfigGroup::from(cg);
        assert!(
            matches!(result, Err(Error::MirrorOfMirror(uuid, target_uuid)) if uuid == "A" && target_uuid == "B"),
            "Failed to detect mirror-of-mirror configuration."
        );
    }

    /// Test that `ValidConfigGroup::from` correctly reports an error when
    /// brightness is outside the allowed range [0.0, 1.0].
    #[test]
    fn test_valid_config_from_invalid_brightness() {
        // Below valid range
        let result_low = ValidConfigGroup::from(ConfigGroup {
            configs: vec![Config {
                uuid: "low-bright".to_owned(),
                brightness: Some(-0.1),
                ..Config::default()
            }],
        });
        assert!(
            matches!(result_low, Err(Error::InvalidBrightness(uuid, value)) if uuid == "low-bright" && value == -0.1),
            "Failed to detect brightness below 0.0."
        );

        // Above valid range
        let result_high = ValidConfigGroup::from(ConfigGroup {
            configs: vec![Config {
                uuid: "high-bright".to_owned(),
                brightness: Some(1.2),
                ..Config::default()
            }],
        });
        assert!(
            matches!(result_high, Err(Error::InvalidBrightness(uuid, value)) if uuid == "high-bright" && value == 1.2),
            "Failed to detect brightness above 1.0."
        );
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Helper to convert configuration groups into a vector of valid
/// configuration groups.  This enforces that no configuration group applies
/// to the same set of UUIDs.  The result will also be sorted from most
/// specific configuration to least specific.
pub fn validate_config_groups(cgs: ConfigGroups) -> Result<Vec<ValidConfigGroup>, Error> {
    // We might be tempted to use a BTreeSet here. However, because
    // incomparable configuration groups with the same number of
    // configurations will be treated as equal, we have to rely on hashing
    // which will correctly distinguish them.
    let mut duplicate_groups = HashSet::new();
    let mut valid_groups = HashSet::new();
    for config_group in cgs.groups {
        let valid_group = ValidConfigGroup::from(config_group)?;
        if valid_groups.contains(&valid_group) {
            duplicate_groups.insert(valid_group);
        } else {
            valid_groups.insert(valid_group);
        }
    }

    // If there are any duplicates report them.
    if !duplicate_groups.is_empty() {
        return Err(Error::DuplicateGroups(duplicate_groups));
    }

    // Order the groups by the most precise first.
    let mut vec_groups: Vec<ValidConfigGroup> = valid_groups.into_iter().collect();
    vec_groups.sort();
    Ok(vec_groups)
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod valid_config_groups_tests {
    use super::*;
    /// Test that `validate_config_groups` detects duplicate configuration groups.
    #[test]
    fn test_config_validation_duplicates() {
        let result = validate_config_groups(ConfigGroups {
            groups: vec![
                ConfigGroup {
                    configs: vec![Config {
                        uuid: "abcdef1234".to_owned(),
                        enabled: Some(false),
                        ..Config::default()
                    }],
                },
                ConfigGroup {
                    configs: vec![Config {
                        uuid: "abcdef1234".to_owned(),
                        enabled: Some(false),
                        ..Config::default()
                    }],
                },
            ],
        });
        assert!(
            matches!(result, Err(Error::DuplicateGroups(groups)) if
                groups.len() == 1 &&
                groups.iter().all(|cg| cg.uuids.len() == 1 && cg.uuids.contains("abcdef1234"))),
            "Failed to detect duplicate configuration groups."
        );

        let result = validate_config_groups(ConfigGroups {
            groups: vec![
                ConfigGroup {
                    configs: vec![
                        Config {
                            uuid: "abcdef1234".to_owned(),
                            enabled: Some(false),
                            ..Config::default()
                        },
                        Config {
                            uuid: "foobarbaz".to_owned(),
                            enabled: Some(false),
                            ..Config::default()
                        },
                    ],
                },
                ConfigGroup {
                    configs: vec![
                        Config {
                            uuid: "foobarbaz".to_owned(),
                            enabled: Some(false),
                            ..Config::default()
                        },
                        Config {
                            uuid: "abcdef1234".to_owned(),
                            enabled: Some(false),
                            ..Config::default()
                        },
                    ],
                },
            ],
        });
        assert!(
            matches!(result, Err(Error::DuplicateGroups(groups)) if
                groups.len() == 1 &&
                groups.iter().all(|cg| cg.uuids.len() == 2 && cg.uuids.contains("abcdef1234") && cg.uuids.contains("foobarbaz"))),
            "Failed to detect duplicate configuration groups."
        );
    }

    /// Test that sorting configuration groups works as expected.
    #[test]
    fn test_config_group_sorting() {
        let configs = vec![
            vec!["a"],
            vec!["b"],
            vec!["c"],
            vec!["a", "b"],
            vec!["a", "c"],
            vec!["a", "b", "c"],
        ];

        fn convert(vec: Vec<&str>) -> ValidConfigGroup {
            ValidConfigGroup {
                uuids: BTreeSet::from_iter(vec.into_iter().map(String::from)),
                configs: HashMap::new(),
            }
        }

        let mut groups: Vec<ValidConfigGroup> = configs.into_iter().map(convert).collect();
        groups.sort();
        let group_uuids: Vec<BTreeSet<String>> =
            groups.iter().map(|vcg| vcg.uuids.clone()).collect();
        assert_eq!(
            format!("{:?}", group_uuids),
            r#"[{"a", "b", "c"}, {"a", "b"}, {"a", "c"}, {"a"}, {"b"}, {"c"}]"#
        );
    }
}
