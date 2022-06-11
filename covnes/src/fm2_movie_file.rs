use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct FM2File {
    pub version: i32,
    pub emu_version: i32,
    pub rerecord_count: Option<i32>,
    pub pal_flag: bool,
    pub new_ppu: bool,
    pub fds: bool,
    pub controllers: ControllerConfiguration,
    pub port2: (),
    pub binary: bool,
    pub length: Option<i32>,
    pub rom_filename: String,
    pub comment: Option<String>,
    pub subtitle: Option<String>,
    pub guid: String,
    pub rom_checksum: String,
    pub savestate: Option<String>,
    pub commands: Vec<Command>,
}

#[derive(Debug, Clone)]
pub enum InputDevice {
    None,
    Gamepad(Vec<GamepadInput>),
    Zapper(Vec<ZapperInput>),
}

#[derive(Debug, Clone)]
pub enum ControllerConfiguration {
    Fourscore(Vec<[GamepadInput; 4]>),
    Ports {
        port0: InputDevice,
        port1: InputDevice,
    },
}

bitflags! {
    pub struct Command : u8 {
        const SOFT_RESET = 0x1;
        const HARD_RESET = 0x2;
        const FDS_DISK_INSERT = 0x4;
        const FDS_DISK_SELECT = 0x8;
        const VS_INSERT_COIN = 0x16;
    }
}

use crate::nes::io::StandardControllerButtons;

pub type GamepadInput = StandardControllerButtons;
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ZapperInput {
    x: u16,
    y: u16,
    mouse_button_pressed: bool,
    q: u8,
    z: u8,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Could not read FM2 file")]
    CouldNotRead(#[from] std::io::Error),

    #[error("No input lines were found")]
    NoInput,

    #[error("Malformed header at line {line_no}")]
    MalformedHeaderLine { line_no: i32 },

    #[error("Duplicate key '{key}' at line {line_no}")]
    DuplicateKey { key: String, line_no: i32 },

    #[error("Required key '{key}' not found in the header")]
    RequiredKeyNotFound { key: &'static str },

    #[error("Key '{key}' is not an integer, it's '{value}'")]
    NotAnInteger { key: &'static str, value: String },

    #[error("Key '{key}' is not an bool (0/1), it's '{value}'")]
    NotABool { key: &'static str, value: i32 },

    #[error("Key '{key}' is not a valid input device")]
    BadInputDevice { key: &'static str },

    #[error("Key '{key}' is not a valid FCExp device")]
    BadPort2 { key: &'static str },

    #[error("File stored using binary format which we don't support")]
    NoBinaryPlease,

    #[error("Wrong number of sections or malformed data on line {line_no}")]
    BadInputLine { line_no: i32 },

    #[error("Bad commands field on line {line_no}")]
    BadCommands { line_no: i32 },

    #[error("Bad gamepad input on line {line_no} for section {section}")]
    BadGamepadInput { line_no: i32, section: &'static str },

    #[error("Bad zapper input on line {line_no} for section {section}")]
    BadZapperInput { line_no: i32, section: &'static str },

    #[error("Bad \"input\" for no connected controller on line {line_no} for section {section}")]
    BadNoInputInput { line_no: i32, section: &'static str },
}

type Result<T, E = Error> = std::result::Result<T, E>;

impl FM2File {
    pub fn parse<R: Read>(f: &mut R) -> Result<FM2File, Error> {
        let mut f = BufReader::new(f);

        // Parse the header key/value
        let mut header_map = HashMap::new();
        let mut line = String::new();
        let mut line_no = 1;
        loop {
            line.clear();
            let read = f.read_line(&mut line)?;

            if read == 0 {
                return Err(Error::NoInput);
            }

            // Trim whitespace
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }

            // Detect end of header
            if line.starts_with('|') {
                break;
            }

            let split: Vec<&str> = line.splitn(2, " ").collect();
            if split.len() != 2 {
                return Err(Error::MalformedHeaderLine { line_no });
            }
            let k = String::from(split[0]);
            let v = String::from(split[1]);

            if header_map.contains_key(&k) {
                return Err(Error::DuplicateKey { key: k, line_no });
            }

            header_map.insert(k, v);

            line_no += 1;
        }

        // Integer keys (also used for booleans, with a 1 for true and 0 for false) must have a value that can be stored as int32:
        //     version (required) - the version of the movie file format; for now it is always 3
        let version = required_int(&mut header_map, "version")?;
        //     emuVersion (required) - the version of the emulator used to produce the movie
        let emu_version = required_int(&mut header_map, "emuVersion")?;
        //     rerecordCount (optional) - the rerecord count
        let rerecord_count = optional_int(&mut header_map, "rerecordCount")?;
        //     palFlag (bool) (optional) - true if the movie uses PAL timing
        let pal_flag = optional_bool_or_false(&mut header_map, "palFlag")?;
        //     NewPPU (bool) (optional) - true if the movie uses New PPU
        let new_ppu = optional_bool_or_false(&mut header_map, "NewPPU")?;
        //     FDS (bool) (optional) - true if the movie was recorded on a Famicom Disk System (FDS) game
        let fds = optional_bool_or_false(&mut header_map, "fds")?;
        //     fourscore (bool) true if a fourscore was used. If fourscore is not used, then port0 and port1 are required

        let fourscore = optional_bool_or_false(&mut header_map, "fourscore")?;

        let mut controllers = {
            if fourscore {
                ControllerConfiguration::Fourscore(Vec::new())
            } else {
                //     port0 - indicates the type of input device attached to the port 0. Supported values are:
                //         SI_NONE = 0
                //         SI_GAMEPAD = 1
                //         SI_ZAPPER = 2
                let port0 = input_device(&mut header_map, "port0")?;
                //     port1 - indicates the type of input device attached to the port 1. Supported values are:
                //         SI_NONE = 0
                //         SI_GAMEPAD = 1
                //         SI_ZAPPER = 2
                let port1 = input_device(&mut header_map, "port1")?;
                ControllerConfiguration::Ports { port0, port1 }
            }
        };
        //     port2 (required) - indicates the type of the FCExp port device which was attached. Supported values are:
        //         SIFC_NONE = 0
        let port2 = {
            let port2 = required_int(&mut header_map, "port2")?;
            if port2 != 0 {
                return Err(Error::BadPort2 { key: "port2" });
            }
            () // Explicitly the value for port2
        };
        //     binary (bool) (optional) - true if input log is stored in binary format
        let binary = optional_bool_or_false(&mut header_map, "fds")?;
        //     length (optional) - movie size (number of frames in the input log). If this key is specified and the number is >= 0, the input log ends after specified number of records, and any remaining data should not be parsed. This key is used in fm3 format to allow storing extra data after the end of input log
        let length = optional_int(&mut header_map, "length")?;

        // String keys have values that consist of the remainder of the key-value pair line. As a
        // consequence, string values cannot contain newlines.
        //
        //     romFilename (required) - the name of the file used to record the movie
        let rom_filename = required(&mut header_map, "romFilename")?;
        //     comment (optional) - simply a memo
        //         by convention, the first token in the comment value is the subject of the comment
        //         by convention, subsequent comments with the same subject should have their ordering preserved and may be used to approximate multi-line comments
        //         by convention, the author of the movie should be stored in comment(s) with a subject of: author
        //         example:
        //             comment author adelikat
        let comment = optional(&mut header_map, "comment");
        //     subtitle (optional) - a message that will be displayed on screen when movie is played back (unless Subtitles are turned off, see Movie options)
        //         by convention, subtitles begin with the word "subtitle"
        //         by convention, an integer value following the word "subtitle" indicates the frame that the subtitle will be displayed
        //         by convention, any remaining text after the integer is considered to be the string displayed
        //         example:
        //             subtitle 1000 Level Two
        //             At frame 1000 the words "Level Two" will be displayed on the screen
        let subtitle = optional(&mut header_map, "subtitle");
        //     guid (required) - a unique identifier for a movie, generated when the movie is created, which is used when loading a savestate to make sure it belongs to the current movie
        //     GUID keys have a value which is in the standard GUID format: 452DE2C3-EF43-2FA9-77AC-0677FC51543B
        let guid = required(&mut header_map, "guid")?;
        //     romChecksum (required) - the base64 of the hexified MD5 hash of the ROM which was used to record the movie (don't ask)
        let rom_checksum = required(&mut header_map, "romChecksum")?;
        //     savestate (optional) - a fcs savestate blob, in case a movie was recorded from savestate
        //     Hex string keys (used for binary blobs) will have a value that is like 0x0123456789ABCDEF...
        let savestate = optional(&mut header_map, "savestate");

        if binary {
            return Err(Error::NoBinaryPlease);
        }

        let mut commands = Vec::new();
        let mut entries = 0;

        loop {
            let parts: Vec<&str> = line.split("|").collect();

            let expected_sections = match controllers {
                ControllerConfiguration::Fourscore(_) => 6,
                ControllerConfiguration::Ports { .. } => 4,
            } + 2;

            if parts.len() != expected_sections {
                return Err(Error::BadInputLine { line_no });
            }

            // Ensure it goes like "|A|B|C|D|"
            if parts[0] != "" || parts[parts.len() - 1] != "" || parts[parts.len() - 2] == "" {
                return Err(Error::BadInputLine { line_no });
            }
            let command = match parts[1].parse::<i32>() {
                Ok(v) => {
                    if v >= 0 {
                        Command::from_bits_truncate((v % 255) as u8)
                    } else {
                        return Err(Error::BadCommands { line_no });
                    }
                }
                Err(_) => return Err(Error::BadCommands { line_no }),
            };

            commands.push(command);

            match &mut controllers {
                ControllerConfiguration::Fourscore(values) => {
                    let p1 = parse_gamepad_input(parts[2], line_no, "player1")?;
                    let p2 = parse_gamepad_input(parts[3], line_no, "player2")?;
                    let p3 = parse_gamepad_input(parts[4], line_no, "player3")?;
                    let p4 = parse_gamepad_input(parts[4], line_no, "player4")?;

                    values.push([p1, p2, p3, p4]);
                }
                ControllerConfiguration::Ports { port0, port1 } => {
                    parse_input_for_input_device(port0, parts[2], line_no, "port0")?;
                    parse_input_for_input_device(port1, parts[3], line_no, "port1")?;
                }
            }

            line_no += 1;
            entries += 1;
            if let Some(x) = length {
                if x == entries {
                    break;
                }
            }

            line.clear();
            let read = f.read_line(&mut line)?;
            if read == 0 {
                break;
            }
            // Trim whitespace
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }
        }

        return Ok(FM2File {
            version,
            emu_version,
            rerecord_count,
            pal_flag,
            new_ppu,
            fds,
            controllers,
            port2,
            binary,
            length,
            rom_filename,
            comment,
            subtitle,
            guid,
            rom_checksum,
            savestate,
            commands,
        });
    }
}

fn optional(map: &mut HashMap<String, String>, key: &'static str) -> Option<String> {
    map.remove(key)
}

fn required(map: &mut HashMap<String, String>, key: &'static str) -> Result<String> {
    match optional(map, key) {
        Some(x) => Ok(x),
        None => Err(Error::RequiredKeyNotFound { key }),
    }
}

fn required_int(map: &mut HashMap<String, String>, key: &'static str) -> Result<i32> {
    let v = required(map, key)?;
    v.parse::<i32>().map_err(|_| Error::NotAnInteger {
        key,
        value: v.clone(),
    })
}

fn optional_int(map: &mut HashMap<String, String>, key: &'static str) -> Result<Option<i32>> {
    match optional(map, key) {
        None => Ok(None),
        Some(v) => v
            .parse::<i32>()
            .map(|x| Some(x))
            .map_err(|_| Error::NotAnInteger {
                key,
                value: v.clone(),
            }),
    }
}

fn optional_bool_or_false(map: &mut HashMap<String, String>, key: &'static str) -> Result<bool> {
    let v = optional_int(map, key)?;
    Ok(match v {
        None => false,
        Some(x) => match x {
            0 => false,
            1 => true,
            _ => return Err(Error::NotABool { key, value: x }),
        },
    })
}

fn input_device(map: &mut HashMap<String, String>, key: &'static str) -> Result<InputDevice> {
    let v = required_int(map, key)?;
    Ok(match v {
        0 => InputDevice::None,
        1 => InputDevice::Gamepad(Vec::new()),
        2 => InputDevice::Zapper(Vec::new()),
        _ => return Err(Error::BadInputDevice { key }),
    })
}

fn parse_gamepad_input(input: &str, line_no: i32, section: &'static str) -> Result<GamepadInput> {
    if input.len() != 8 {
        return Err(Error::BadGamepadInput { line_no, section });
    }
    let mut v = 0u8;
    for c in input.chars() {
        v <<= 1;
        match c {
            '.' | ' ' => (),
            _ => v |= 1,
        }
    }

    Ok(GamepadInput::from_bits_truncate(v))
}

fn parse_zapper_input(_input: &str, _line_no: i32, _section: &'static str) -> Result<ZapperInput> {
    unimplemented!();
}

fn parse_no_controller_input_input(input: &str, line_no: i32, section: &'static str) -> Result<()> {
    if input != "" {
        return Err(Error::BadNoInputInput { line_no, section });
    }
    Ok(())
}

fn parse_input_for_input_device(
    input_device: &mut InputDevice,
    input: &str,
    line_no: i32,
    section: &'static str,
) -> Result<()> {
    match input_device {
        InputDevice::None => {
            parse_no_controller_input_input(input, line_no, section)?;
        }
        InputDevice::Gamepad(entries) => {
            entries.push(parse_gamepad_input(input, line_no, section)?);
        }
        InputDevice::Zapper(entries) => {
            entries.push(parse_zapper_input(input, line_no, section)?);
        }
    }

    Ok(())
}
