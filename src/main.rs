#[macro_use]
extern crate trackable;

#[macro_use]
extern crate lazy_static;

use clap::{App, Arg};
use dialoguer::{theme::ColorfulTheme, Select};
use indicatif::ProgressBar;
use m3u8_rs::{playlist::MasterPlaylist, playlist::MediaPlaylist, playlist::Playlist};
use mpeg2ts::ts::{ReadTsPacket, TsPacketReader, TsPacketWriter, WriteTsPacket};
use std::{fs::File, io::Read, path::Path};

lazy_static! {
    static ref CLIENT: reqwest::Client = reqwest::Client::new();
}

fn process_master_playlist(mp: &MasterPlaylist) {
    let mut variants: Vec<&String> = Vec::new();
    let mut uris: Vec<&String> = Vec::new();

    for variant in &mp.variants {
        match &variant.resolution {
            Some(r) => {
                variants.push(&r);
                uris.push(&variant.uri);
            }
            _ => {}
        }
    }

    variants.reverse();
    uris.reverse();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose a resolution to download")
        .default(0)
        .items(&variants[..])
        .interact()
        .unwrap();
    let m3u8 = download_m3u8(uris[selection]).unwrap();
    load_m3u8(m3u8);
}

fn process_media_playlist(mp: &MediaPlaylist) {
    let pb = ProgressBar::new(mp.segments.len() as u64);
    let output_file = File::create("output.ts").unwrap();
    let mut writer = TsPacketWriter::new(output_file);
    for segment in &mp.segments {
        let buffer = download_ts(&segment.uri);
        let mem = match buffer {
            Ok(b) => b,
            e => panic!("Error when downloading stream blobs: {:#?}", e),
        };
        let mut reader = TsPacketReader::new(mem.as_slice());
        while let Some(packet) = track_try_unwrap!(reader.read_ts_packet()) {
            track_try_unwrap!(writer.write_ts_packet(&packet));
        }
        pb.inc(1);
    }
    pb.finish_with_message("Finished downloading!")
}

fn download_ts(uri: &String) -> Result<Vec<u8>, reqwest::Error> {
    let mut buffer: Vec<u8> = Vec::new();
    CLIENT.get(uri).send()?.read_to_end(&mut buffer).unwrap();
    Ok(buffer)
}

fn download_m3u8(uri: &String) -> Result<Vec<u8>, reqwest::Error> {
    let text = reqwest::get(uri)?.text()?;
    Ok(Vec::from(text.as_bytes()))
}

fn load_m3u8(bytes: Vec<u8>) {
    match m3u8_rs::parse_playlist_res(&bytes) {
        Ok(Playlist::MasterPlaylist(pl)) => process_master_playlist(&pl),
        Ok(Playlist::MediaPlaylist(pl)) => process_media_playlist(&pl),
        Err(e) => println!("Error: {:#?}", e),
    }
}

fn load_file<P: AsRef<Path>>(path: P) {
    let mut file = File::open(path).unwrap();
    let mut bytes: Vec<u8> = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    load_m3u8(bytes);
}

fn main() {
    let matches = App::new("m3u8-dl")
        .version("0.101")
        .about("Downloads m3u8 playlist video/audio from file or net")
        .arg(
            Arg::with_name("file")
                .short("f")
                .long("file")
                .value_name("FILE")
                .help("Load a m3u8 file from the file system"),
        )
        .get_matches();

    let file = matches.value_of("file").unwrap_or("");
    match file {
        "" => {}
        f => load_file(f),
    }
}
