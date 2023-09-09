///! This module provides a data structure that is allows for more efficient
/// access to a configuration group that has been validated as being
/// semantically consistent.  That is, it doesn't have duplicate displays
/// and is non-empty.
use std::collections::{BTreeSet, HashMap, HashSet};

use std::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use std::fmt::Formatter;
use std::hash::{Hash, Hasher};

use crate::config::*;

/// The possible errors that can ar
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
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::DuplicateDisplays(uuids) => {
                write!(
                    f,
                    "A configuration group contains a configuration for \
                the same display more than once: {}",
                    uuids
                        .into_iter()
                        .cloned()
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
            Error::DuplicateGroups(configs) => {
                // Collect up all the groups of uuids that are duplicated.
                let dups = configs
                    .iter()
                    .map(|vc| {
                        let uuids = vc.uuids.iter().cloned().collect::<Vec<String>>().join(", ");
                        let mut group = String::from("[");
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
        }
    }
}

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
    /// more elements, or are more "precise", to be "smaller".
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        use Ordering::*;
        match (
            self.uuids.is_superset(&other.uuids),
            other.uuids.is_superset(&self.uuids),
        ) {
            (true, true) => Some(Equal),
            (true, false) => Some(Less),
            (false, true) => Some(Greater),
            _ => None,
        }
    }
}

impl Ord for ValidConfigGroup {
    /// There cannot be a true total ordering on configuration groups,
    /// but this definition should be sufficient for sorting configurations
    /// by precision.  However, given that two incomparable configuration
    /// groups of the same length are effectively treated as equal, if
    /// there are duplicates, there is no guarantee the truly identical
    /// configuration groups will be clustered together.
    fn cmp(&self, other: &Self) -> Ordering {
        // If they are incomparable use their size.
        self.partial_cmp(other)
            .unwrap_or(other.uuids.len().cmp(&self.uuids.len()))
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
            let uuid = config.uuid.clone();
            if configs.contains_key(&uuid) {
                duplicates.insert(uuid);
            } else {
                configs.insert(uuid, config);
            }
        }

        // If there are any duplicate displays report the error.
        if !duplicates.is_empty() {
            Err(Error::DuplicateDisplays(duplicates))
            // A group must have at least one Config.
        } else if configs.is_empty() {
            Err(Error::EmptyGroup)
        } else {
            Ok(ValidConfigGroup {
                uuids: configs.keys().cloned().collect(),
                configs,
            })
        }
    }
}

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
