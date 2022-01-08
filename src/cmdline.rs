use crate::{device::DeviceSelector, score::Channels};
use std::{num::ParseIntError, path::PathBuf, time::Duration};

use structopt::StructOpt;

#[derive(StructOpt)]
pub struct Cli {
    // TODO: `conflicts_with` doesn't seem to work!
    #[structopt(
        short = "r",
        long = "rec-device-num",
        conflicts_with = "rec_device_name"
    )]
    pub rec_device_num: Option<usize>,
    #[structopt(
        short = "R",
        long = "rec-device-name",
        conflicts_with = "rec_device_num"
    )]
    pub rec_device_name: Option<String>,
    #[structopt(
        short = "p",
        long = "play-device-num",
        conflicts_with = "rec_device_name"
    )]
    pub play_device_num: Option<usize>,
    #[structopt(
        short = "P",
        long = "play-device-name",
        conflicts_with = "rec_device_num"
    )]
    pub play_device_name: Option<String>,
    #[structopt(
        short = "d",
        long = "delay",
        parse(try_from_str = parse_duration),
        default_value="0",
    )]
    pub delay: Duration,
    #[structopt(short = "i", long = "--input-score-file", parse(from_os_str))]
    pub input_score_file: PathBuf,
    #[structopt(short = "c", long = "--input-channels")]
    pub input_channels: Vec<Channels>,
    #[structopt(short = "C", long = "--output-channels")]
    pub output_channels: Vec<Channels>,
    #[structopt(short = "o", long = "--playback-score-file", parse(from_os_str))]
    pub playback_score_file: PathBuf,
}

fn parse_duration(src: &str) -> Result<Duration, ParseIntError> {
    let millis = src.parse()?;
    Ok(Duration::from_millis(millis))
}

pub fn parse_args() -> (Cli, DeviceSelector, DeviceSelector) {
    let args = Cli::from_args();
    let device = match (args.rec_device_num, args.rec_device_name.clone()) {
        (Some(rec_device_num), None) => DeviceSelector::Number(rec_device_num),
        (None, Some(rec_device_name)) => DeviceSelector::NameSubstring(rec_device_name),
        _ => {
            panic!("-r/--rec-device or -R/--rec-device-name required")
        }
    };
    let playback_device = match (args.play_device_num, args.play_device_name.clone()) {
        (Some(play_device_num), None) => DeviceSelector::Number(play_device_num),
        (None, Some(play_device_name)) => DeviceSelector::NameSubstring(play_device_name),
        _ => {
            panic!("-p/--play-device or -P/--play-device-name required")
        }
    };
    (args, device, playback_device)
}
