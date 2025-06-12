//! This module provides a data structure that is allows for more efficient
//! access to a configuration group that has been validated as being
//! semantically consistent.  That is, it doesn't have duplicate displays
//! and is non-empty.
use coverage_helper::test;
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
                {
                    return Err(Error::InvalidMirrorConfig(uuid.clone()));
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
fn test_valid_config_from_empty() {
    match ValidConfigGroup::from(ConfigGroup { configs: vec![] }) {
        Err(Error::EmptyGroup) => { /* Correctly detected error, so no-op */ }
        Err(_) => panic!("Unexpected error in validation."),
        Ok(_) => panic!("Failed to detect empty configuration."),
    }
}

/// Check that `ValidConfigGroup::from` correctly reports an error for
/// duplicate configurations.
#[test]
fn test_valid_config_from_duplicate() {
    match ValidConfigGroup::from(ConfigGroup {
        configs: vec![
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
            },
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
            },
        ],
    }) {
        Err(Error::DuplicateDisplays(uuids)) => {
            assert_eq!(uuids.len(), 1);
            assert!(uuids.contains("abcdef1234"))
        }
        Err(_) => panic!("Unexpected error in validation."),
        Ok(_) => panic!("Failed to detect duplicate configuration."),
    }

    match ValidConfigGroup::from(ConfigGroup {
        configs: vec![
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
            },
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
            },
            Config {
                uuid: "foobarbaz".to_owned(),
                mirror_of: None,
                enabled: Some(false),
                origin: None,
                extents: None,
                scaled: None,
                frequency: None,
                color_depth: None,
                rotation: None,
            },
            Config {
                uuid: "foobarbaz".to_owned(),
                mirror_of: None,
                enabled: Some(false),
                origin: None,
                extents: None,
                scaled: None,
                frequency: None,
                color_depth: None,
                rotation: None,
            },
        ],
    }) {
        Err(Error::DuplicateDisplays(uuids)) => {
            assert_eq!(uuids.len(), 2);
            assert!(uuids.contains("abcdef1234"));
            assert!(uuids.contains("foobarbaz"));
        }
        Err(_) => panic!("Unexpected error in validation."),
        Ok(_) => panic!("Failed to detect duplicate configuration."),
    }
}

#[cfg(test)]
mod valid_config_group_tests {
    use super::*;
    use crate::displays::Point;
    use coverage_helper::test;

    /// Test that `ValidConfigGroup::from` correctly reports an error for
    /// mirror_of with incompatible display options.
    #[test]
    fn test_valid_config_from_invalid_mirror() {
        match ValidConfigGroup::from(ConfigGroup {
            configs: vec![Config {
                uuid: "abcdef1234".to_owned(),
                mirror_of: Some("5678defghi".to_owned()),
                enabled: Some(true), // enabled is allowed with mirror_of
                origin: Some(Point { x: 0, y: 0 }), // origin is not allowed with mirror_of
                extents: None,
                scaled: None,
                frequency: None,
                color_depth: None,
                rotation: None,
            }],
        }) {
            Err(Error::InvalidMirrorConfig(uuid)) => {
                assert_eq!(uuid, "abcdef1234");
            }
            Err(_) => panic!("Unexpected error in validation."),
            Ok(_) => panic!("Failed to detect invalid mirror configuration."),
        }
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
                    origin: None,
                    extents: None,
                    scaled: None,
                    frequency: None,
                    color_depth: None,
                    rotation: None,
                },
                Config {
                    uuid: "B".to_owned(),
                    mirror_of: Some("C".to_owned()),
                    enabled: Some(true),
                    origin: None,
                    extents: None,
                    scaled: None,
                    frequency: None,
                    color_depth: None,
                    rotation: None,
                },
                Config {
                    uuid: "C".to_owned(),
                    mirror_of: None,
                    enabled: Some(true),
                    origin: None,
                    extents: None,
                    scaled: None,
                    frequency: None,
                    color_depth: None,
                    rotation: None,
                },
            ],
        };

        match ValidConfigGroup::from(cg) {
            Err(Error::MirrorOfMirror(uuid, target_uuid)) => {
                assert_eq!(uuid, "A");
                assert_eq!(target_uuid, "B");
            }
            Err(_) => panic!("Unexpected error in mirror-of-mirror validation."),
            Ok(_) => panic!("Failed to detect mirror-of-mirror configuration."),
        }
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
mod valid_config_groups_tests {
    use super::*;
    use coverage_helper::test;
    /// Test that `validate_config_groups` detects duplicate configuration groups.
    #[test]
    fn test_config_validation_duplicates() {
        match validate_config_groups(ConfigGroups {
            groups: vec![
                ConfigGroup {
                    configs: vec![Config {
                        uuid: "abcdef1234".to_owned(),
                        mirror_of: None,
                        enabled: Some(false),
                        origin: None,
                        extents: None,
                        scaled: None,
                        frequency: None,
                        color_depth: None,
                        rotation: None,
                    }],
                },
                ConfigGroup {
                    configs: vec![Config {
                        uuid: "abcdef1234".to_owned(),
                        mirror_of: None,
                        enabled: Some(false),
                        origin: None,
                        extents: None,
                        scaled: None,
                        frequency: None,
                        color_depth: None,
                        rotation: None,
                    }],
                },
            ],
        }) {
            Err(Error::DuplicateGroups(groups)) => {
                assert_eq!(groups.len(), 1);
                assert!(groups
                    .iter()
                    .all(|cg| cg.uuids.len() == 1 && cg.uuids.contains("abcdef1234")))
            }
            Err(_) => panic!("Unexpected error in validation."),
            Ok(_) => panic!("Failed to detect duplicate configuration groups."),
        }

        match validate_config_groups(ConfigGroups {
            groups: vec![
                ConfigGroup {
                    configs: vec![
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
                        },
                        Config {
                            uuid: "foobarbaz".to_owned(),
                            mirror_of: None,
                            enabled: Some(false),
                            origin: None,
                            extents: None,
                            scaled: None,
                            frequency: None,
                            color_depth: None,
                            rotation: None,
                        },
                    ],
                },
                ConfigGroup {
                    configs: vec![
                        Config {
                            uuid: "foobarbaz".to_owned(),
                            mirror_of: None,
                            enabled: Some(false),
                            origin: None,
                            extents: None,
                            scaled: None,
                            frequency: None,
                            color_depth: None,
                            rotation: None,
                        },
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
                        },
                    ],
                },
            ],
        }) {
            Err(Error::DuplicateGroups(groups)) => {
                assert_eq!(groups.len(), 1);
                assert!(groups.iter().all(|cg| cg.uuids.len() == 2
                    && cg.uuids.contains("abcdef1234")
                    && cg.uuids.contains("foobarbaz")));
            }
            Err(_) => panic!("Unexpected error in validation."),
            Ok(_) => panic!("Failed to detect duplicate configuration groups."),
        }
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
