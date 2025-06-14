use clap::{Arg, ArgAction, ArgMatches, Command};
use humantime;
use log::*;
use serde::Serialize;
use simplelog::{SharedLogger, TermLogger, WriteLogger};
use std::collections::HashMap;
use std::fmt::Formatter;
use std::io::IsTerminal;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::{Condvar, LazyLock, Mutex, RwLock};

use crate::config::*;
use crate::core_graphics;
use crate::displays;
use crate::displays::*;
use crate::indirect_logger::IndirectLogger;
use crate::serde::serialize_to_string;
use crate::valid_config;
use crate::valid_config::*;

////////////////////////////////////////////////////////////////////////////////

/// Representation of all the possible failure modes.
#[derive(Debug)]
pub enum Error {
    // Wrapper errors.
    Argument(clap::Error),
    Config(valid_config::Error),
    Displays(displays::Error),
    Io(std::io::Error),
    Utf8(std::string::FromUtf8Error),
    Serde(crate::serde::Error),
    Duration(humantime::DurationError),
    LogInit(SetLoggerError),

    // knoll module errors.
    NoConfigGroups,
    // TODO For these errors fall back to storing the configuration as a
    //   string because we cannot thread the configured serialization
    //   format through `std::fmt::Display`.
    NoMatchingConfigGroup(Vec<String>),
    NoMatchingDisplayMode(String, String),
    AmbiguousDisplayMode(String, Vec<String>),
    AmbiguousConfigGroup(Vec<String>),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        use crate::knoll::Error::*;
        match self {
            Argument(e) => Some(e),
            Config(e) => Some(e),
            Displays(e) => Some(e),
            Io(e) => Some(e),
            Utf8(e) => Some(e),
            Serde(e) => Some(e),
            Duration(e) => Some(e),
            LogInit(e) => Some(e),
            _ => None,
        }
    }
}

impl From<clap::Error> for Error {
    fn from(e: clap::Error) -> Self {
        Error::Argument(e)
    }
}

impl From<valid_config::Error> for Error {
    fn from(e: valid_config::Error) -> Self {
        Error::Config(e)
    }
}

impl From<displays::Error> for Error {
    fn from(e: displays::Error) -> Self {
        Error::Displays(e)
    }
}

impl From<crate::serde::Error> for Error {
    fn from(e: crate::serde::Error) -> Self {
        Error::Serde(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(e: std::string::FromUtf8Error) -> Self {
        Error::Utf8(e)
    }
}

impl From<humantime::DurationError> for Error {
    fn from(e: humantime::DurationError) -> Self {
        Error::Duration(e)
    }
}

impl From<SetLoggerError> for Error {
    fn from(e: SetLoggerError) -> Self {
        Error::LogInit(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use crate::knoll::Error::*;

        match self {
            // TODO Detect if the output is a terminal to determine when
            //  to use ANSI escape codes.
            Argument(ce) => write!(f, "{}", ce.render().ansi()),
            NoConfigGroups => write!(
                f,
                "The parsed input contains no configuration groups.  \
            Daemon mode requires at least one configuration group."
            ),
            NoMatchingConfigGroup(uuids) => {
                write!(
                    f,
                    "No configuration group matches these currently attached displays: {}.",
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
            NoMatchingDisplayMode(uuid, str) => {
                write!(
                    f,
                    "No display mode matches the given configuration for {}: {}",
                    uuid, str
                )
            }
            AmbiguousDisplayMode(uuid, str) => {
                write!(
                    f,
                    "Ambiguous choice of display mode for {}: {}",
                    uuid,
                    str.join(" ")
                )
            }
            Config(ce) => {
                write!(f, "{}", ce)
            }
            Displays(de) => {
                write!(f, "{}", de)
            }
            Serde(se) => se.fmt(f),
            Utf8(ue) => write!(f, "Invalid UTF-8 in input: {}", ue),
            Duration(de) => write!(f, "Invalid wait period duration: {}", de),
            LogInit(le) => write!(f, "Error initializing logger: {}", le),

            // TODO Not specific enough to determine input versus output error?
            //   Introduce an additional wrapper?
            Io(ie) => write!(f, "I/O error: {}", ie),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Helper to convert a verbosity magnitude into a logging `LevelFilter`.
/// Verbosity of `0` corresponds to only logging `Error`s.
fn verbosity_to_filter(verbosity: usize) -> LevelFilter {
    match verbosity {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    }
}

/// A handle to the current global IndirectLogger.
static GLOBAL_LOGGER: LazyLock<RwLock<Option<IndirectLogger>>> =
    LazyLock::new(|| RwLock::new(None));

/// Helper to configure the logger by verbosity and depending on whether it
/// is writing to a terminal or not.
fn configure_logger<ERR: Write + IsTerminal + Send + 'static>(
    verbosity: usize,
    stderr: ERR,
) -> Result<(), SetLoggerError> {
    let mut config_builder = simplelog::ConfigBuilder::new();
    config_builder.set_time_format_rfc3339();

    let level_filter = verbosity_to_filter(verbosity);
    let session_logger: Box<dyn SharedLogger> = if stderr.is_terminal() {
        // If the destination is a terminal, use the `Termlogger`.
        TermLogger::new(
            level_filter,
            config_builder.build(),
            simplelog::TerminalMode::Stderr,
            simplelog::ColorChoice::Auto,
        )
    } else {
        // Otherwise just use a plain `WriteLogger`.
        WriteLogger::new(level_filter, config_builder.build(), stderr)
    };

    // Update or initialize the global logger.
    let mut opt_logger = GLOBAL_LOGGER.write().unwrap();
    match opt_logger.as_mut() {
        Some(logger) => logger.update(session_logger),
        None => {
            *opt_logger = Some(IndirectLogger::init(session_logger)?);
        }
    }

    Ok(())
}

/// Helper to flush the current logger, if it exists.
fn flush_logger() {
    // Flush the current logger, if it exists.
    if let Some(logger) = GLOBAL_LOGGER.read().unwrap().as_ref() {
        logger.flush();
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Generic entry point to the knoll command-line tool.  
/// It is parameterized by the DisplayState implementation as well as
/// the input, output, and error targets.
pub fn run<
    'l,
    DS: DisplayState,
    IN: Read + IsTerminal,
    OUT: Write + 'l,
    ERR: Write + IsTerminal + Send + 'static,
>(
    args: &Vec<String>,
    stdin: IN,
    stdout: OUT,
    stderr: ERR,
) -> Result<(), Error> {
    // Handle parsing the command-line arguments.
    let matches = argument_parse(args)?;

    // Examine the serialization format option.
    let format_opt: Option<&str> = matches.get_one::<String>("FORMAT").map(|s| s.as_str());
    // TODO Seems like there should be a function that lifts Option to Result?
    let format = match format_opt {
        Some(fs) => crate::serde::Format::from(fs)?,
        // This error should have be caught during argument parsing.
        _ => panic!("Invalid serialization format"),
    };

    // Set up logging.
    let verbosity = matches.get_count("VERBOSITY").into();
    configure_logger(verbosity, stderr)?;

    // Check to see which program mode should be used.
    let result = match matches.subcommand() {
        Some(("daemon", sub_matches)) => {
            info!("Daemon mode selected.");

            let config_reader =
                ConfigReader::new(format, stdin, sub_matches.get_one::<PathBuf>("IN"))?;

            // Calling unwrap here should be okay, as there is a default value.
            let wait_string = sub_matches.get_one::<String>("WAIT").unwrap();
            let wait_period = humantime::parse_duration(wait_string)?;
            let exit_after_first = sub_matches.get_flag("EXIT");
            daemon_command::<DS>(config_reader, format, wait_period, exit_after_first)
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
            let config_reader = ConfigReader::new(format, stdin, matches.get_one::<PathBuf>("IN"))?;
            let mut output = open_output(stdout, matches.get_one::<PathBuf>("OUT"))?;

            pipeline_command::<DS>(quiet, config_reader, output.as_mut(), format)
        }
    };

    // Ensure that all logging is flushed before exiting.
    flush_logger();

    result
}

/// Helper for parsing the command-line arguments.
fn argument_parse(args: &Vec<String>) -> Result<ArgMatches, clap::Error> {
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
    // Option solely for testing purposes, so hidden.
    let daemon_exit_arg = Arg::new("EXIT")
        .help("Exit the daemon after the first reconfiguration event")
        .hide(true)
        .long("exit")
        .short('e')
        .action(ArgAction::SetTrue);

    let cmd = Command::new("knoll")
        .version(clap::crate_version!())
        .about("Tool for configuring and arranging displays")
        .args(vec![quiet_arg, verbose_arg, format_arg])
        .args(&file_args)
        .subcommands([
            Command::new("daemon")
                .about("Run in daemon mode updating when the hardware configuration changes")
                .arg(in_arg)
                .arg(wait_arg)
                .arg(daemon_exit_arg),
            Command::new("list")
                .about("Print information about available display modes")
                .arg(out_arg),
        ]);

    cmd.try_get_matches_from(args)
}

////////////////////////////////////////////////////////////////////////////////

/// `ConfigReader` is a helper to abstract away from reading the configuration,
/// rather than just reading it directly.  Daemon mode takes advantage of this
/// so that it is possible to update the configuration without having to restart
/// knoll.  However, it will only be able to reload if the input is specified as
/// a file.  If there was no file input and `stdin` is to be use, then it will
/// be read once and subsequent calls to `groups()` will yield the same
/// configuration.  If `stdin` happens to be a terminal, rather than a pipe,
/// etc. the result will be empty.
struct ConfigReader {
    /// Format to use when deserializing configurations.
    format: crate::serde::Format,
    /// Optional path to reload the configurations from.
    opt_path: Option<PathBuf>,
    /// Current configurations.
    config_string: String,
}

impl ConfigReader {
    /// Create a new `ConfigReader` given the file format, the current `stdin`
    /// and possibly a path to read a configuration from.
    fn new<IN: Read + IsTerminal>(
        format: crate::serde::Format,
        stdin: IN,
        opt_path: Option<&PathBuf>,
    ) -> Result<Self, Error> {
        let config_string = match opt_path {
            // If we are reading from a file, we can skip reading it here,
            // as we'll reload it every time the configuration is requested.
            Some(_) => String::new(),
            None => {
                // If stdin is a terminal rather than a redirect, do not try to
                // read from it.  Otherwise, BufRead may block forever waiting
                // for data.
                if stdin.is_terminal() {
                    String::new()
                // We cannot reload stdin, so read it now.  This also simplifies
                // the lifetime of the ConfigReader.
                } else {
                    let mut buffer = Vec::new();
                    let _ = BufReader::new(stdin).read_to_end(&mut buffer)?;
                    String::from_utf8(buffer)?
                }
            }
        };

        Ok(Self {
            opt_path: opt_path.cloned(),
            config_string,
            format,
        })
    }

    /// Parse and validate configuration groups.  If the `ConfigReader` was
    /// created with an input file, this will reload the configurations
    /// groups from that file first.
    fn groups(&mut self) -> Result<Vec<ValidConfigGroup>, Error> {
        // If the configuration is being read from a file, reload it now.
        match &self.opt_path {
            Some(path) => self.config_string = std::fs::read_to_string(path)?,
            None => { /* No-op */ }
        }

        // If the input is empty return no configuration groups, as
        // deserialization will fail.
        if self.config_string.is_empty() {
            return Ok(vec![]);
        }

        // Deserialize and validate the configurations.
        Ok(validate_config_groups(crate::serde::deserialize(
            self.format,
            self.config_string.as_str(),
        )?)?)
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Helper for handling the output argument.  It no output path was provided,
/// stdout will be used instead.  Will return a boxed `BufWrite` that can be
/// used to write the program output.
fn open_output<'l, OUT: Write + 'l>(
    stdout: OUT,
    opt_path: Option<&PathBuf>,
) -> std::io::Result<Box<dyn Write + 'l>> {
    let output: Box<dyn Write> = match opt_path {
        Some(path) => Box::new(std::fs::File::open(path)?),
        None => Box::new(stdout),
    };

    Ok(output)
}

////////////////////////////////////////////////////////////////////////////////

/// Helper find the configuration group for the current display state.
// TODO Detect when configuration change would be a no-op.
fn find_most_precise_config_group<DS: DisplayState>(
    vcgs: &[ValidConfigGroup],
    display_state: &DS,
    format: crate::serde::Format,
) -> Result<ValidConfigGroup, Error> {
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
        Err(Error::NoMatchingConfigGroup(
            displays.keys().cloned().collect(),
        ))
    }
    // Ambiguous configurations.
    else if matching.len() > 1 {
        // TODO A little annoying that it is necessary to use a loop rather
        // than mapping so that ? can be used.
        let mut cg_strs = Vec::new();
        for vcg in matching {
            let cg = ConfigGroup {
                configs: vcg.configs.values().cloned().collect(),
            };
            cg_strs.push(serialize_to_string(format, &cg)?)
        }
        Err(Error::AmbiguousConfigGroup(cg_strs))
    } else {
        // Okay to unwrap here as we have verified that there is
        // at least one match.
        Ok(matching.pop().unwrap().clone())
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Helper to convert a `Config` to `DisplayModePattern`
fn mode_pattern_from_config(config: &Config) -> DisplayModePattern {
    DisplayModePattern {
        scaled: config.scaled,
        color_depth: config.color_depth,
        frequency: config.frequency,
        extents: config.extents.clone(),
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Helper to select a matching display mode for the given display
/// using the requested configuration.
/// Will fail if there is no matching display mode, or if the configuration
/// does not uniquely determine a display mode.
fn select_mode<D: Display>(
    display: &D,
    config: &Config,
    format: crate::serde::Format,
) -> Result<D::DisplayModeType, Error> {
    let pattern = mode_pattern_from_config(config);
    let mut modes = display.matching_modes(&pattern);
    if modes.len() > 1 {
        // If there is more than one matching mode, the configuration is ambiguous.

        let mut mode_strs = Vec::new();
        // TODO A little annoying that it is necessary to use a loop rather
        // than mapping so that ? can be used.
        for m in modes {
            mode_strs.push(serialize_to_string(format, &m)?)
        }
        Err(Error::AmbiguousDisplayMode(
            display.uuid().to_string(),
            mode_strs,
        ))
    } else if let Some(mode) = modes.pop() {
        // If we can pop a mode, then there is exactly one match, and we can use it.
        Ok(mode)
    } else {
        // Otherwise there was no match.
        Err(Error::NoMatchingDisplayMode(
            display.uuid().to_string(),
            serialize_to_string(format, &config)?,
        ))
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Configure displays from configuration group.
fn configure_displays<DS: DisplayState>(
    display_state: &DS,
    config_group: ValidConfigGroup,
    format: crate::serde::Format,
) -> Result<(), Error> {
    // Determine that we can find appropriate display modes for each
    // configuration before we start configuring.
    let mut selected_modes = HashMap::new();
    let displays = display_state.get_displays();

    for (uuid, config) in &config_group.configs {
        // Skip selecting a mode for mirrored displays; they will inherit the
        // mirrored display's mode
        if config.mirror_of.is_some() {
            continue;
        }
        let display = displays
            .get(uuid)
            .expect("Match display somehow missing display configuration");
        let mode = select_mode(display, config, format)?;
        info!(
            "For display {}, selected mode {}",
            &uuid,
            serialize_to_string(format, &mode)?
        );
        selected_modes.insert(uuid.clone(), mode);
    }

    let mut cfgtxn = display_state.configure()?;
    for (uuid, config) in &config_group.configs {
        // First handle mirroring configurations.
        if let Some(mirror_of_uuid) = &config.mirror_of {
            info!(
                "Setting display {} to mirror display {}",
                uuid, mirror_of_uuid
            );
            cfgtxn.set_mirroring(uuid, Some(mirror_of_uuid))?;
            // When mirroring is set, we skip other configuration options except 'enabled'
            if let Some(false) = config.enabled {
                info!("Display {} has been disabled.", uuid);
                cfgtxn.set_enabled(uuid, false)?;
            }
            // As the display will inherit the remaining options,
            // we can continue on to the next display.
            continue;
        }
        // Proceed with non-mirroring configuration.
        if let Some(false) = config.enabled {
            info!("Display {} has been disabled.", uuid);
            cfgtxn.set_enabled(uuid, false)?;
            // Skip the rest of the configuration for disabled displays
            continue;
        }

        // If not mirroring, disable.
        info!("Disabling mirroring for display {}", uuid);
        cfgtxn.set_mirroring(uuid, None)?;

        // TODO roll back rotation if later steps fail?
        if let Some(rotation) = config.rotation {
            info!(
                "For display {}, using rotation of {} degrees.",
                uuid, rotation
            );
            cfgtxn.set_rotation(uuid, rotation)?
        }

        // TODO roll back brightness if later steps fail?
        if let Some(brightness) = config.brightness {
            info!(
                "For display {}, setting brightness to {}.",
                uuid, brightness
            );
            cfgtxn.set_brightness(uuid, brightness)?;
        }

        // Unwrap is safe as we know there is a display mode for each UUID.
        cfgtxn.set_mode(uuid, selected_modes.get(uuid).unwrap())?;

        if let Some(origin) = &config.origin {
            info!("For display {}, using {} as origin.", uuid, origin);
            cfgtxn.set_origin(uuid, origin)?
        }
    }

    cfgtxn.commit()?;
    info!("Configuration complete.");

    Ok(())
}

////////////////////////////////////////////////////////////////////////////////

/// Helper to convert a given display state into configuration groups.
fn state_to_config<DS: DisplayState>(display_state: &DS) -> ConfigGroups {
    let configs: Vec<Config> = display_state
        .get_displays()
        .iter()
        .map(|(uuid, display)| {
            let config = Config {
                uuid: uuid.clone(),
                ..Config::default()
            };
            let mirror_of = display.mirror_of().map(|s| s.to_string());
            if mirror_of.is_some() {
                Config {
                    mirror_of: mirror_of,
                    ..config
                }
            } else {
                let mode = display.current_mode();
                Config {
                    enabled: Some(display.enabled()),
                    origin: Some(display.origin().clone()),
                    extents: Some(mode.extents().clone()),
                    scaled: Some(mode.scaled()),
                    frequency: Some(mode.frequency()),
                    color_depth: Some(mode.color_depth()),
                    rotation: Some(display.rotation()),
                    brightness: display.brightness(),
                    ..config
                }
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
    mut config_reader: ConfigReader,
    output: &mut dyn Write,
    format: crate::serde::Format,
) -> Result<(), Error> {
    let mut display_state = DS::current()?;

    let config_groups = config_reader.groups()?;

    // If there are any configuration groups, attempt to apply them.
    if !config_groups.is_empty() {
        let chosen_config = find_most_precise_config_group(&config_groups, &display_state, format)?;
        configure_displays(&display_state, chosen_config, format)?;
        // Update the display state with any changes that were applied.
        display_state = DS::current()?;
    }

    // Unless quieted, write the display state to the output
    if !quiet {
        let cgs = state_to_config(&display_state);
        crate::serde::serialize(format, &cgs, output)?;
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
    format: crate::serde::Format,
) -> Result<(), Error> {
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
    crate::serde::serialize(format, &groups, output)?;

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

/// Atomic counter to keep track of the number of reconfigurations that have
/// taken place.
static RECONFIGURE_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Helper to acquire the reconfiguration lock and notify the conditional
/// variable.  It can also be used a callback for when displace notification
/// changes.
extern "C" fn triger_reconfig() {
    if let Ok(ref mut reconfig_started) = RECONFIGURE_LOCK.try_lock() {
        **reconfig_started = true;
        // Signal to the worker thread to wake up and perform
        // the reconfiguration.
        RECONFIGURE_CONDVAR.notify_one();
    }
}

fn daemon_command<DS: DisplayState>(
    mut config_reader: ConfigReader,
    format: crate::serde::Format,
    wait_period: std::time::Duration,
    exit_after_first: bool,
) -> Result<(), Error> {
    // Spawn a thread to watch for reconfiguration changes.
    std::thread::spawn(move || {
        'loop_label: loop {
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

            // Wait for the display configuration to quiesce.
            std::thread::sleep(wait_period);
            info!("Reconfiguring displays.");

            // As close as I think we can get to monadic binding.
            let result = config_reader
                .groups()
                .and_then(|config_groups: Vec<ValidConfigGroup>| {
                    if config_groups.is_empty() {
                        Err(Error::NoConfigGroups)
                    } else {
                        DS::current()
                            .map_err(|e| e.into())
                            .and_then(|display_state: DS| {
                                let current_config = state_to_config(&display_state);
                                let config_str = serialize_to_string(format, &current_config).expect(
                                    "Should be impossible to fail on serializing internally constructed configuration.",
                                );
                                info!("Current display state:\n{}", config_str);

                                find_most_precise_config_group(&config_groups, &display_state, format).and_then(
                                    |config_group: ValidConfigGroup| {
                                        configure_displays(&display_state, config_group, format)
                                    },
                                )
                            })
                    }
                });

            match result {
                Err(e) => {
                    error!("{}", e);
                }
                Ok(()) => {
                    info!("Reconfiguration successful.");
                }
            };

            // Reconfiguration has completed.
            *reconfig_in_progress = false;
            // Increment the reconfiguration counter.
            RECONFIGURE_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    });

    // Install the display reconfiguration callback.
    core_graphics::cg_display_register_reconfiguration_callback(triger_reconfig);

    // Trigger an initial reconfiguration.  This is to handle the case that you
    // have knoll running as a launchd service, and as macOS starts up your
    // monitor configuration is incorrect even before knoll is started.
    triger_reconfig();

    if !exit_after_first {
        // macOS will not trigger the callback unless there is an application
        // loop running.
        core_graphics::ns_application_load();
        core_graphics::cf_run_loop_run();
    } else {
        // Otherwise, wait for the reconfiguration counter to increment.
        while RECONFIGURE_COUNT.load(std::sync::atomic::Ordering::SeqCst) == 0 {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }

    Ok(())
}
