# knoll

<p>
<a href="https://crates.io/crates/knoll"><img src="https://img.shields.io/crates/v/knoll?style=flat-square" alt="Crates.io version" /></a>
<img src="https://github.com/gawashburn/knoll/actions/workflows/tests.yml/badge.svg" alt="Testing action" />
<a href="https://coveralls.io/github/gawashburn/knoll"><img src="https://coveralls.io/repos/github/gawashburn/knoll/badge.svg" alt="Coverage report" /></a>
<img src="https://img.shields.io/github/license/gawashburn/knoll" alt="MIT License" />
</p>

A simple command-line tool for manipulating the configuration of macOS displays.

## Table of contents

- [Installation](#installation)
    - [Cargo](#cargo)
    - [launchd](#launchd)
    - [Nix](#nix)
- [Usage](#usage)
    - [Pipeline mode](#pipeline-mode)
    - [Listing mode](#listing-mode)
    - [Daemon mode](#daemon-mode)
- [Configuration reference](#configuration-reference)
- [Future work](#future-work)
- [Development](#development)
- [What's in a name?](#whats-in-a-name)

## Installation

Until someone creates packages for knoll, probably the most common way to
install it will be to use cargo or Nix.

### Cargo

If you already have a Rust environment set up, you can use the
`cargo install` command:

```bash
cargo install knoll
```

### launchd

The recommended solution for running knoll as a daemon is to make use of
[
`launchd`](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html).
Choose a service name unique to your host using
the [reverse domain name](https://en.wikipedia.org/wiki/Reverse_domain_name_notation)
convention and create a `.plist` file in `~/Library/LaunchAgents`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple Computer//DTD PLIST 1.0//EN"
        "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
    <dict>
        <key>EnvironmentVariables</key>
        <dict>
            <key>PATH</key>
            <string>...</string>
        </dict>
        <key>KeepAlive</key>
        <true/>
        <key>Label</key>
        <string>my.service.knoll</string>
        <key>ProgramArguments</key>
        <array>
            <string>/path/to/knoll</string>
            <string>daemon</string>
            <string>-vvv</string>
            <string>--input=/path/to/config-file</string>
        </array>
        <key>RunAtLoad</key>
        <true/>
        <key>StandardErrorPath</key>
        <string>/tmp/knoll.err</string>
        <key>StandardOutPath</key>
        <string>/tmp/knoll.out</string>
    </dict>
</plist>
```

You can then enable and start service using

```bash
launchctl enable gui/$(id -u)/my.service.knoll`
launchctl start gui/$(id -u)/my.service.knoll`
````

### Nix

The knoll repository contains a [Nix Flake](https://nixos.wiki/wiki/Flakes)
that can be used to integrate knoll into your
[nix-darwin](https://github.com/LnL7/nix-darwin/) configuration. I currently
use the following `launchd` definition like:

```nix
  launchd.user.agent = {
    knoll = {
      path = [ "/run/current-system/sw/bin/" ];
      serviceConfig = {
        ProgramArguments = let
          configFile = pkgs.writeText "knoll-config.json"
            (builtins.toJSON [
              [
                # MacBook Pro display
                {
                  uuid = "8684ad81e3ea92cb14f43eb88b97a3f7";
                  enabled = true;
                  origin = [ (-1792) 453 ];
                  extents = [ 1792 1120 ];
                  scaled = true;
                  frequency = 59;
                  color_depth = 8;
                  rotation = 0;
                }
                ...
              ]
            ]);
        in
          [
            "/run/current-system/sw/bin/knoll" "daemon" "-vvv" "--format=json"
            "--input=${configFile}"
          ];
        KeepAlive = true;
        RunAtLoad = true;
        StandardErrorPath = "/tmp/knoll.err";
        StandardOutPath = "/tmp/knoll.out";
      };
    };
  };
```

## Usage

knoll has three primary usage modes: pipeline mode, listing mode, and
daemon mode.

### Pipeline mode

knoll's default mode supports reporting and updating the current display
configuration. In the simplest case, you can just run it with no argument:

```bash
host$ knoll
[
  [
    {
      "uuid": "b00184f4c1ee4cdf8ccfea3fca2f93b2",
      "enabled": true,
      "origin": [
        0,
        0
      ],
      "extents": [
        2560,
        1440
      ],
      "scaled": true,
      "frequency": 60,
      "color_depth": 8,
      "rotation": 0
    }
  ]
]
```

The output here is the current display configuration
in [JSON](https://www.json.org/)
format. It says that there is a single enabled display placed at (0,0) with a
scaled resolution of 2560x1440. The display is not rotated and has a refresh
frequency of 60Hz and a color depth of 8-bits.

knoll also supports
[Rusty Object Notation (RON)](https://github.com/ron-rs/ron).

```bash
host$ knoll --format=ron
[
    [
        (
            uuid: "b00184f4c1ee4cdf8ccfea3fca2f93b2",
            enabled: true,
            origin: (0, 0),
            extents: (2560, 1440),
            scaled: true,
            frequency: 60,
            color_depth: 8,
            rotation: 0,
        ),
    ],
]
```

There are two primary benefits of using RON over JSON. One is that it is a
slightly more compact. Second, and more importantly, it supports comments. This
way you can annotate your configurations if you like. JSON was chosen as the
default as it makes it easier to interface knoll with all the tooling available
as part of the JSON ecosystem.

You may have noticed that the display configuration is nested two levels deep.
knolls output consists of an outermost list of *configuration groups*. Each
configuration group in turn consists of a list of display configurations.

By default, knoll will read a list of configuration groups from standard
input and apply the most specific configuration group that is applicable.

As the output of knoll is a configuration group, piping
knoll to itself is an idempotent operation:

```bash
host$ knoll | knoll --quiet
# Should not change anything.
```

Note that because the operating system may accept some configuration changes
without failure, but modifying them to satisfy certain constraints, providing
knoll with a configuration is not an identity:

```bash
host$ cat my_config.json | knoll > out_config.json 
# my_config.json and out_config.json may differ.
```

The most common case where this might happen is that `my_config.json` omits
some fields we are not interested in adjusting. Another case where this
might happen would be if a configuration group has displays that overlap or
have gaps. We will call these *unstable* configurations.

As just mentioned, display configurations can omit any fields that you do not
want to alter. For example, if you just wanted to rotate your display to be
upside-down, you could write the following:

```bash
host$ cat my_config.ron
[
    [
        (
            uuid: "b00184f4c1ee4cdf8ccfea3fca2f93b2",
            rotation: 180,
        ),
    ],
]
host$ knoll --quiet --format=ron --input=my_config.ron
```

The resolution, location, etc. of the display will all remain unchanged.

The only required field is `uuid`. If just the `uuid` field
is provided the configuration is effectively a no-op.

Earlier I glossed over what it means for knoll to choose a "most specific"
configuration group. A valid configuration group consists of one or more
display configurations with unique UUIDs:

```bash
[   // This is an invalid configuration group because
    // there are duplicate UUIDs.
    (   // First configuration
        uuid: "b00184f4c1ee4cdf8ccfea3fca2f93b2",
    ),
    (   // Second configuration
        uuid: "b00184f4c1ee4cdf8ccfea3fca2f93b2",
    )
]
```

A valid list of configuration groups must contain only groups that do not have
the same set of UUIDs.

```bash
[   // This is an invalid list of configuration groups because 
    // there are two groups with the same set of UUIDs.
    [ // First group
        (
            uuid: "b00184f4c1ee4cdf8ccfea3fca2f93b2",
        ),
    ],
    [   // Second group
        (
            uuid: "b00184f4c1ee4cdf8ccfea3fca2f93b2",
        )
    ],
]
```

Given these restrictions on validity, when run, knoll will determine all the
UUID of all attached displays. It will then choose the configuration group
where its UUIDs are the largest subset of the attached displays. The intent is
here is two-fold:

* Attaching a new display to the computer will not cause an existing
  configuration to become invalid.
* It is possible to provide configurations with and without this new display.

If there is no applicable display group in the provided configuration,
knoll will exit with an error message and error code:

```bash
host$ cat bogus.ron
[
    [
        ( // Improbable display UUID.
          uuid: "11111111111111111111111111111111",
        ),
    ],
]
host$ knoll --quiet --format=ron --input=bogus.ron
No configuration group matches the currently attached displays: 
37d8832a2d6602cab9f78f30a301b230, 94226c6fcef04e9b8503ffa88fedba08,
f3def94a9fbd4de79a432d9d0bc7b4ce.
host$ echo $?
1
```

### Listing mode

knoll's second mode of operation allows inspecting the allowed display mode of
attached displays:

```bash
host$ knoll list
[
  {
    "uuid": "37d8832a2d6602cab9f78f30a301b230",
    "modes": [
      {
        "scaled": true,
        "color_depth": 8,
        "frequency": 59,
        "extents": [
          1280,
          800
        ]
      },

      {
        "scaled": true,
        "color_depth": 8,
        "frequency": 60,
        "extents": [
          1024,
          768
        ]
      }
    ]
  }
]
```

This is useful for determining which display configurations may successfully be
used in an input to knoll.

### Daemon mode

Finally, knoll also supports a "daemon" mode.

```bash
host$ knoll daemon --input=my_config.json
```

When in this mode, knoll wait until a display configuration event occurs. At
that time, if provided an input file, it will (re)load the configuration from
the file specified in the input argument. It will then choose an applicable
configuration group, should one exist, and apply it. However, if no
applicable group is found, it will not exit with an error.

Either way, knoll will continue to run and wait for a display reconfiguration
event from the operating system. At that point it will wait a few seconds for
the configuration to settle, and then attempt to find a matching configuration
and apply it.

Note, that while knoll can still accept a piped configuration, because of the
nature of pipes, it will not be able to reload the configuration upon a
reconfiguration event.

This quiescence period is to avoid knoll from triggering during some fumbling
with cables, quickly opening and closing a laptop lid, or displays taking some
time to awaken from sleep. If the default period is too long for your desired
level of responsiveness, it can be configured:

```bash
host$ knoll daemon --wait=500ms --input=my_config.json
```

## Configuration reference

A configuration may contain the following fields:

* `uuid`  In JSON: `"uuid": "b00184f4c1ee4cdf8ccfea3fca2f93b2"`. In RON
  `uuid: "b00184f4c1ee4cdf8ccfea3fca2f93b2"`. This is used to uniquely identify
  a given display. This is the only required field.
* `enabled`  In JSON `"enabled": true`. In RON `enabled: true`. In knolls
  output this indicates whether display is enabled, and in the input indicates
  whether it should remain enabled. Due to limitations in the APIs knoll uses
  at present, disabling a display will remove it from the computer's
  configuration. So once disabled, it can only be re-enabled by unplugging
  the display, restarting, etc.
* `origin`  In JSON `"origin": [ 100, 100 ]`. In RON `origin: (100, 100)`. This
  specifies the current or requested location of the display's upper left
  corner.  
  Displays may not overlap and all displays must touch.
* `extents`  In JSON `"extents": [ 2560, 1440 ]`  In RON
  `extends: (2560, 1440)`. This
  specifies either the current or requested resolution of the display.
* `scaled`  In JSON `"scaled": true`. In RON `scaled: true`. This specifies
  whether
  the current or requested display mode should use one-to-one pixels or a
  "scaled" ("Retina") mode.
* `frequency`  In JSON `"frequency": 60`. In RON `frequency: 60`. This
  specifies the current or requested refresh frequency for the display in Hertz.
* `color_depth`  In JSON `"color_depth": 8`. In RON `color_depth: 8`. This
  specifies the current or requested color depth of the display.
* `rotation`  In JSON `"rotation": 90`. In RON `rotation: 90`. This
  specifies the current or requested rotation of the display in degrees. At
  present, only 0, 90, 180, and 270 degree rotations are supported.

## Future work

So far knoll has been working successfully for my specific use cases. However,
there is still room for additional improvements:

* Bug fixing. There remain many strange new displays to explore.
* Writing more tests.
* Support for display mirroring. I only ever mirror displays for presentations,
  so I opted to punt on this for the first release. There is already some
  initial internals in place to support mirroring, but plumbing and testing is
  still needed.
* Find a better API for enabling/disabling displays. Most users would expect
  this feature to put the display to sleep rather than detach it from the
  computer.
* Detect display configurations with overlapping displays or gaps to at warn
  that the configuration is not stable.
* Support UUID abbreviations similar to git hash abbreviations.
* Support configuring the brightness, gamma function, etc. for a display.
* Cannot easily write tests against logged output as `stderrlog` does not
  currently provide a way to control where it sends output.
* It seems plausible that knoll could be extended to support Windows, XOrg,
  Wayland, etc. It is just a matter of finding the appropriate APIs and perhaps
  making some additional generalizations to the configuration data structures.

## Development

<p>
<a href="https://blog.rust-lang.org/2023/01/10/Rust-1.83.0.html">
    <img src="https://img.shields.io/badge/rustc-1.83.0+-lightgray.svg" alt="Rust 1.83.0+" />
</a>
<a href="https://github.com/gawashburn/knoll/blob/master/LICENCE">
    <img src="https://img.shields.io/badge/licence-MIT-green" alt="MIT Licence" />
</a>
</p>

knoll is written in [Rust](https://www.rust-lang.org/). I have not attempted
cross-compilation, but at present it seems unlikely that knoll could be compiled
successfully on another operating system other than macOS. That said, knoll
does not actually depend on any macOS headers, etc. so it should be possible
to compile it without installing
[XCode](https://developer.apple.com/xcode/).

Pull requests are definitely welcome. I am still a Rust novice, so it also
entirely possible there are better or more idiomatic ways to write some of
this code. I have endeavoured to write knoll in a way that is conducive to
unit testing. So please try to add appropriate tests for submitted changes.

## What's in a name?

knoll's name derives from the term
[knolling](https://en.wikipedia.org/wiki/|knolling):
> Kromelow would arrange any displaced tools at right angles on all surfaces,
> and called this routine knolling, in that the tools were arranged in right
> angles ... The result was an organized surface that allowed the user
> to see all objects at once.

It seemed apt as macOS does not currently support placing displays at arbitrary
angles and most users will want to organize their displays to all be clearly
visible.
