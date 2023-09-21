use clap::{Arg, ArgAction, ArgMatches, Command};
use humantime;
use log::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Formatter;
use std::io::IsTerminal;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::{Condvar, Mutex};
use stderrlog;

use crate::config::*;
use crate::core_graphics;
use crate::displays;
use crate::displays::*;
use crate::valid_config;
use crate::valid_config::*;

////////////////////////////////////////////////////////////////////////////////

/// Representation of all the possible failure modes.
pub enum KnollError {
    NoConfigGroups,
    EmptyConfiguration,

    // TODO For these errors fall back to storing the configuration as a
    //   string because we cannot thread the configured serialization
    //   format through `std::fmt::Display`.
    NoMatchingConfigGroup(Vec<String>),
    NoMatchingDisplayMode(String),
    AmbiguousDisplayMode(Vec<String>),
    AmbiguousConfigGroup(Vec<String>),

    Config(valid_config::Error),
    Displays(displays::Error),
    Io(std::io::Error),
    DeRon(ron::error::SpannedError),
    SerRon(ron::error::Error),
    SerdeJson(serde_json::Error),
    Duration(humantime::DurationError),
}

impl From<valid_config::Error> for KnollError {
    fn from(e: valid_config::Error) -> Self {
        KnollError::Config(e)
    }
}

impl From<displays::Error> for KnollError {
    fn from(e: displays::Error) -> Self {
        KnollError::Displays(e)
    }
}

impl From<std::io::Error> for KnollError {
    fn from(e: std::io::Error) -> Self {
        KnollError::Io(e)
    }
}

impl From<ron::error::SpannedError> for KnollError {
    fn from(e: ron::error::SpannedError) -> Self {
        KnollError::DeRon(e)
    }
}

impl From<ron::error::Error> for KnollError {
    fn from(e: ron::error::Error) -> Self {
        KnollError::SerRon(e)
    }
}

impl From<serde_json::Error> for KnollError {
    fn from(e: serde_json::Error) -> Self {
        KnollError::SerdeJson(e)
    }
}

impl From<humantime::DurationError> for KnollError {
    fn from(e: humantime::DurationError) -> Self {
        KnollError::Duration(e)
    }
}

impl std::fmt::Display for KnollError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use KnollError::*;

        match self {
            NoConfigGroups => write!(
                f,
                "The parsed input contains no configuration groups.  \
            Daemon mode requires at least one configuration group."
            ),
            EmptyConfiguration => write!(f, "Input is empty."),
            NoMatchingConfigGroup(uuids) => {
                write!(
                    f,
                    "No configuration group matches the currently attached displays: {}.",
                    uuids.join(", ")
                )
            }
            AmbiguousConfigGroup(str) => {
                write!(
                    f,
                    "Ambiguous choice of configurations groups: {}",
                    str.join(" ")
                )
            }
            NoMatchingDisplayMode(str) => {
                write!(
                    f,
                    "No display mode matches the given configuration: {}",
                    str
                )
            }
            AmbiguousDisplayMode(str) => {
                write!(f, "Ambiguous choice of display mode: {}", str.join(" "))
            }
            Config(ce) => {
                write!(f, "{}", ce)
            }
            Displays(de) => {
                write!(f, "{}", de)
            }
            // TODO Not specific enough to determine input versus output error?
            //   Introduce an additional wrapper?
            Io(ie) => write!(f, "I/O error: {}", ie),
            DeRon(se) => write!(f, "RON deserialization error: {}", se),
            SerRon(se) => write!(f, "RON serialization error: {}", se),
            // TODO Separate out serialization and deserialization errors.
            SerdeJson(se) => write!(f, "JSON (de)seserialization error: {}", se),
            Duration(de) => write!(f, "Invalid wait period duration: {}", de),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Tag class to specify the kind of (de)serialization to be used.
#[derive(Debug, Clone, Copy)]
enum DataFormat {
    Ron,
    Json,
}

/// Helper to abstract over serialization, parameterized by the selected
/// data format.
fn generic_serialize<S: Serialize, W: Write>(
    format: DataFormat,
    s: &S,
    writer: W,
) -> Result<(), KnollError> {
    match format {
        DataFormat::Ron => {
            let pretty_config = ron::ser::PrettyConfig::new();
            // TODO For some inexplicable reason there is an asymmetry in RON's
            // serialization to
            ron::ser::to_writer_pretty(writer, s, pretty_config)?
        }
        DataFormat::Json => serde_json::ser::to_writer_pretty(writer, s)?,
    }
    Ok(())
}

/// Helper to abstract over deserialization, parameterized by the selected
/// data format.
fn generic_deserialize<'a, D: Deserialize<'a>>(
    format: DataFormat,
    str: &'a str,
) -> Result<D, KnollError> {
    Ok(match format {
        DataFormat::Ron => ron::de::from_str(str)?,
        DataFormat::Json => serde_json::from_str(str)?,
    })
}

////////////////////////////////////////////////////////////////////////////////

/// Generic entry point to the knoll command-line tool.  
/// It is parameterized by the DisplayState implementation as well as
/// the input and out.
pub fn run<'l, DS: DisplayState, IN: Read + IsTerminal, OUT: Write>(
    args: &Vec<String>,
    stdin: &'l mut IN,
    stdout: &'l mut OUT,
) -> Result<(), KnollError> {
    // Handle parsing the command-line arguments.
    let matches = argument_parse(args);

    // Examine the serialization format option.
    let format_opt: Option<&str> = matches.get_one::<String>("FORMAT").map(|s| s.as_str());
    let format = match format_opt {
        Some("ron") => DataFormat::Ron,
        Some("json") => DataFormat::Json,
        // This error should have be caught during argument parsing.
        _ => panic!("Invalid serialization format"),
    };

    // Set up logging.
    stderrlog::new()
        .module(module_path!())
        .verbosity(matches.get_count("VERBOSITY") as usize)
        .show_level(false)
        .timestamp(stderrlog::Timestamp::Off)
        .init()
        .unwrap();

    // Check to see which program mode should be used.
    match matches.subcommand() {
        Some(("daemon", sub_matches)) => {
            info!("Daemon mode selected.");

            let opt_input = open_input(stdin, sub_matches.get_one::<PathBuf>("IN"))?;
            let config_groups = match opt_input {
                Some(input) => read_config_groups::<IN>(input, format)?,
                None => vec![],
            };
            // Must have at least one configuration group for daemon mode.
            if config_groups.is_empty() {
                return Err(KnollError::NoConfigGroups);
            }
            // Calling unwrap here should be okay, as there is a default value.
            let wait_string = sub_matches.get_one::<String>("WAIT").unwrap();
            let wait_period = humantime::parse_duration(wait_string)?;

            daemon_command::<DS>(config_groups, wait_period)
        }
        Some(("list", sub_matches)) => {
            info!("List mode selected.");

            let mut output = open_output(stdout, sub_matches.get_one::<PathBuf>("OUT"))?;
            list_command::<DS>(output.as_mut(), format)
        }
        _ => {
            info!("Pipeline mode selected.");

            // Should we print the resulting configuration?
            let quiet = matches.get_flag("QUIET");
            let opt_input = open_input(stdin, matches.get_one::<PathBuf>("IN"))?;
            let config_groups = match opt_input {
                Some(input) => read_config_groups::<IN>(input, format)?,
                None => vec![],
            };
            let mut output = open_output(stdout, matches.get_one::<PathBuf>("OUT"))?;

            pipeline_command::<DS>(quiet, config_groups, output.as_mut(), format)
        }
    }
}

/// Helper for parsing the command-line arguments.
fn argument_parse(args: &Vec<String>) -> ArgMatches {
    // Clap argument parsing setup.

    let in_arg = Arg::new("IN")
        .help("File to read from instead of standard input")
        .long("input")
        .short('i')
        .value_parser(clap::value_parser!(std::path::PathBuf));
    let out_arg = Arg::new("OUT")
        .help("File to write to instead of standard output")
        .long("output")
        .short('o')
        .value_parser(clap::value_parser!(std::path::PathBuf));
    let file_args = [in_arg.clone(), out_arg.clone()];

    let quiet_arg = Arg::new("QUIET")
        .short('q')
        .long("quiet")
        .help("Do not write to output, just provide an exit code")
        .action(ArgAction::SetTrue);

    let verbose_arg = Arg::new("VERBOSITY")
        .short('v')
        .long("verbosity")
        .help("Increase verbosity of information emitted to stderr")
        .action(ArgAction::Count)
        .global(true);
    let format_arg = Arg::new("FORMAT")
        .long("format")
        .help("Choose serialization format")
        .default_value("json")
        .value_parser(["json", "ron"])
        .global(true);

    let wait_arg = Arg::new("WAIT")
        .help("Home long to wait after a reconfiguation event to update")
        .long("wait")
        .short('w')
        .default_value("2s")
        .value_parser(clap::builder::NonEmptyStringValueParser::new());

    let cmd = Command::new("knoll")
        .version("0.1.0")
        .about("Tool for configuring and arranging displays")
        .args(vec![quiet_arg, verbose_arg, format_arg])
        .args(&file_args)
        .subcommands([
            Command::new("daemon")
                .about("Run in daemon mode updating when the hardware configuration changes")
                .arg(in_arg)
                .arg(wait_arg),
            Command::new("list")
                .about("Print information about available display modes")
                .arg(out_arg),
        ]);

    cmd.get_matches_from(args)
}

/// Helper for handling the input argument.  If no input path was provided
/// stdin will be used, unless it is the terminal rather than a pipe, etc.
/// In which case None will be returned, to single the program was not
/// provided an input.  Otherwise, a boxed `BufRead` will be returned that
/// can be used to read the input.
fn open_input<'l, IN: Read + IsTerminal>(
    stdin: &'l mut IN,
    opt_path: Option<&PathBuf>,
) -> std::io::Result<Option<Box<dyn BufRead + 'l>>> {
    let input: Option<Box<dyn BufRead>> = match opt_path {
        Some(path) => Some(Box::new(BufReader::new(std::fs::File::open(path)?))),
        None => {
            // If stdin is a terminal rather than a redirect, do not try to
            // read from it.  Otherwise, BufRead may block forever waiting
            // for data.
            if stdin.is_terminal() {
                None
            } else {
                Some(Box::new(BufReader::new(stdin)))
            }
        }
    };

    Ok(input)
}

/// Helper for handling the output argument.  It no output path was provided,
/// stdout will be used instead.  Will return a boxed `BufWrite` that can be
/// used to write the program output.
fn open_output<'l, OUT: Write>(
    stdout: &'l mut OUT,
    opt_path: Option<&PathBuf>,
) -> std::io::Result<Box<dyn Write + 'l>> {
    let output: Box<dyn Write> = match opt_path {
        Some(path) => Box::new(std::fs::File::open(path)?),
        None => Box::new(stdout),
    };

    Ok(output)
}

/// Helper to read configuration groups in the given serialization format
/// from the input.  Can either fail due to the input being empty or
/// there being a mistake in the input such that it cannot be deserializd.
fn read_config_groups<'l, IN: Read + IsTerminal>(
    input: Box<dyn BufRead + 'l>,
    format: DataFormat,
) -> Result<Vec<ValidConfigGroup>, KnollError> {
    // TODO Use `read_to_end` instead?
    // Accumulate lines of output.
    let mut input_str = String::new();
    for line_result in input.lines() {
        input_str.push_str(line_result?.as_str());
        input_str.push('\n');
    }

    if input_str.is_empty() {
        return Err(KnollError::EmptyConfiguration);
    }

    let cgs: ConfigGroups = generic_deserialize(format, input_str.as_str())?;

    Ok(validate_config_groups(cgs)?)
}

/// Helper find the configuration group for the current display state.
/// TODO Detect when configuration change would be a no-op.
fn find_most_precise_config_group<DS: DisplayState>(
    vcgs: &Vec<ValidConfigGroup>,
    display_state: &DS,
) -> Result<ValidConfigGroup, KnollError> {
    let displays = display_state.get_displays();
    let num_displays = displays.len();

    let mut matching = Vec::new();
    let mut best_len = 0;

    for valid_group in vcgs {
        let group_len = valid_group.uuids.len();
        // Only proceed if the config has at most as many displays
        // as there are currently, if it has at least as many displays as
        // the current best and all of the configs correspond to one of the
        // active displays.
        if group_len <= num_displays
            && best_len <= group_len
            && valid_group.uuids.iter().all(|c| displays.contains_key(c))
        {
            // If the new group is larger than the current best, then
            // we can eliminate all the current matches.
            if best_len < group_len {
                matching.clear();
                best_len = group_len;
            }
            matching.push(valid_group.clone());
        }
    }

    // No matching configurations
    if best_len == 0 {
        Err(KnollError::NoMatchingConfigGroup(
            // TODO
            displays.keys().cloned().collect(),
        ))
    }
    // Ambiguous configurations.
    else if matching.len() > 1 {
        // TODO Should use selected serialization format rather the debug
        //   format.
        Err(KnollError::AmbiguousConfigGroup(
            matching.into_iter().map(|cg| format!("{:?}", cg)).collect(),
        ))
    } else {
        // Okay to unwrap here as we have verified that there is
        // at least one match.
        Ok(matching.pop().unwrap().clone())
    }
}

/// Helper to convert a `Config` to `DisplayModePattern`
fn mode_pattern_from_config(config: &Config) -> DisplayModePattern {
    DisplayModePattern {
        scaled: config.scaled,
        color_depth: config.color_depth,
        frequency: config.frequency,
        extents: config.extents.clone(),
    }
}

/// Helper to select a matching display mode for the given display
/// using the requested configuration.
/// Will fail if there is no matching display mode, or if the configuration
/// does not uniquely determine a display mode.
fn select_mode<D: Display>(display: &D, config: &Config) -> Result<D::DisplayModeType, KnollError> {
    let pattern = mode_pattern_from_config(config);
    let mut modes = display.matching_modes(&pattern);
    if modes.is_empty() {
        // TODO Should use selected serialization format rather the debug
        //   format.
        Err(KnollError::NoMatchingDisplayMode(format!("{:?}", config)))
    } else if modes.len() > 1 {
        // TODO Should use selected serialization format rather the debug
        //   format.
        Err(KnollError::AmbiguousDisplayMode(
            modes.into_iter().map(|m| format!("{:?}", m)).collect(),
        ))
    } else {
        // The unwrap here is safe we as we've established that set of matching
        // modes is non-empty.
        Ok(modes.pop().unwrap())
    }
}

/// Configure displays from configuration group.
fn configure_displays<DS: DisplayState>(
    display_state: &DS,
    config_group: ValidConfigGroup,
) -> Result<(), KnollError> {
    // Determine that we can find appropriate display modes for each
    // configuration before we start configuring.
    let mut selected_modes = HashMap::new();
    for (uuid, config) in &config_group.configs {
        let display = display_state
            .get_displays()
            .get(uuid)
            .expect("Match display somehow missing display configuration");
        let mode = select_mode(display, config)?;
        debug!("Selected mode {:?}", mode);
        selected_modes.insert(uuid.clone(), mode);
    }

    let mut cfg = display_state.configure()?;
    for (uuid, config) in &config_group.configs {
        if config.enabled.is_some() {
            // Unwrap is okay as we just checked that there is a value.
            let enabled = config.enabled.unwrap();
            // TODO Use inspect_err to invoke cancel when it becomes available?
            cfg.set_enabled(uuid, enabled)?;
            // TODO Does it make sense to skip the rest?
            if !enabled {
                continue;
            }
        }

        // TODO roll back rotation if later steps fail?
        if config.rotation.is_some() {
            // Unwrap is okay as we just checked that there is a value.
            // TODO Use inspect_err to invoke cancel when it becomes available?
            cfg.set_rotation(uuid, config.rotation.unwrap())?
        }

        // Unwrap is safe as we know there is a display mode for each UUID.
        // TODO Use inspect_err to invoke cancel when it becomes available?
        cfg.set_mode(uuid, selected_modes.get(uuid).unwrap())?;

        if config.origin.is_some() {
            // Unwrap is okay as we just checked that there is a value.
            // TODO Use inspect_err to invoke cancel when it becomes available?
            cfg.set_origin(uuid, config.origin.as_ref().unwrap())?
        }
    }

    Ok(cfg.commit()?)
}

/// Helper to convert a given display state into configuration groups.
fn state_to_config<DS: DisplayState>(display_state: &DS) -> ConfigGroups {
    let configs: Vec<Config> = display_state
        .get_displays()
        .iter()
        .map(|(uuid, display)| {
            let mode = display.current_mode();
            Config {
                uuid: uuid.clone(),
                // TODO Implement mirroring support.
                mirrors: HashSet::new(),
                enabled: Some(display.enabled()),
                origin: Some(display.origin().clone()),
                extents: Some(mode.extents().clone()),
                scaled: Some(mode.scaled()),
                frequency: Some(mode.frequency()),
                color_depth: Some(mode.color_depth()),
                rotation: Some(display.rotation()),
            }
        })
        .collect();

    ConfigGroups {
        groups: vec![ConfigGroup { configs }],
    }
}

////////////////////////////////////////////////////////////////////////////////

fn pipeline_command<DS: DisplayState>(
    quiet: bool,
    config_groups: Vec<ValidConfigGroup>,
    output: &mut dyn Write,
    format: DataFormat,
) -> Result<(), KnollError> {
    let mut display_state = DS::current()?;

    // If there are any configuration groups, attempt to apply them.
    if !config_groups.is_empty() {
        let chosen_config = find_most_precise_config_group(&config_groups, &display_state)?;
        configure_displays(&display_state, chosen_config)?;
        // Update the display state with any changes that were applied.
        display_state = DS::current()?;
    }

    // Unless quieted, write the display state to the output
    if !quiet {
        let cgs = state_to_config(&display_state);
        generic_serialize(format, &cgs, output)?;
    }

    Ok(())
}

////////////////////////////////////////////////////////////////////////////////

/// Helper structure for serializing display modes.
#[derive(Debug, PartialEq, Eq, Clone, Default, Serialize)]
struct DisplayModeGroup<DM>
where
    DM: Serialize,
{
    uuid: String,
    modes: Vec<DM>,
}

fn list_command<DS: DisplayState>(
    output: &mut dyn Write,
    format: DataFormat,
) -> Result<(), KnollError> {
    let display_state = DS::current()?;

    let mut groups: Vec<DisplayModeGroup<DS::DisplayModeType>> = Vec::new();

    // Collect up all modes.
    for (uuid, display) in display_state.get_displays() {
        groups.push(DisplayModeGroup {
            uuid: uuid.clone(),
            modes: Vec::from(display.possible_modes()),
        });
    }

    // Serialize them to output.
    generic_serialize(format, &groups, output)?;

    Ok(())
}

////////////////////////////////////////////////////////////////////////////////

// Reconfiguration in daemon mode is guarded by a lock.  Depending on the
// system configuration, doing something a simple as opening a closed laptop
// lid will trigger multiple invocations of the callback.  To prevent those
// from needlessly triggering reconfiguration multiple times, we use a mutex
// over a Boolean and signal via a condition variable that the reconfiguration
// worker thread should wake up.
static RECONFIGURE_LOCK: Mutex<bool> = Mutex::new(false);
static RECONFIGURE_CONDVAR: Condvar = Condvar::new();

fn daemon_command<DS: DisplayState>(
    config_groups: Vec<ValidConfigGroup>,
    wait_period: std::time::Duration,
) -> Result<(), KnollError> {
    extern "C" fn recon_callback() {
        match RECONFIGURE_LOCK.try_lock() {
            Ok(ref mut reconfig_started) => {
                **reconfig_started = true;
                // Signal to the worker thread to wake up and perform
                // the reconfiguration.
                RECONFIGURE_CONDVAR.notify_one();
            }
            _ => {
                // No-op, as reconfiguration is already taking place.
            }
        }
    }

    // Spawn a thread to watch for reconfiguration changes.
    std::thread::spawn(move || 'loop_label: loop {
        let mut reconfig_in_progress = match RECONFIGURE_LOCK.lock() {
            Ok(mutex) => mutex,
            Err(pe) => {
                error!("Error obtaining reconfiguration lock: {}", pe);
                continue;
            }
        };

        // Wait for the callback to notify that reconfiguration should take place.
        while !*reconfig_in_progress {
            reconfig_in_progress = match RECONFIGURE_CONDVAR.wait(reconfig_in_progress) {
                Ok(b) => b,
                Err(pe) => {
                    error!(
                        "Error while waiting for a reconfiguration notification: {}",
                        pe
                    );
                    continue 'loop_label;
                }
            }
        }

        // Wait for the display configuration to quiese.
        std::thread::sleep(wait_period);
        info!("Reconfiguring displays.");
        match DS::current() {
            Err(de) => {
                error!("{}", de)
            }
            Ok(display_state) => {
                match find_most_precise_config_group(&config_groups, &display_state) {
                    Err(ke) => {
                        error!("{}", ke)
                    }
                    Ok(config_group) => {
                        match configure_displays(&display_state, config_group) {
                            Err(ke) => {
                                error!("{}", ke)
                            }
                            Ok(()) => { /* Reconfiguration succeeded, no-op */ }
                        }
                    }
                }
            }
        }

        // Reconfiguration has completed.
        *reconfig_in_progress = false;
    });

    core_graphics::cg_display_register_reconfiguration_callback(recon_callback);

    // macOS will not trigger the callback unless there is an application
    // loop running.
    core_graphics::ns_application_load();
    core_graphics::cf_run_loop_run();

    Ok(())
}
