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

struct M3U8 {
    input_buffer: Vec<u8>,
    output_file_name: String,
}

impl M3U8 {
    /// Creation

    // Process a M3U8 path in the file system
    pub fn from_fs<P: AsRef<Path>, U: Into<String>>(path: P, ofn: U) -> Self {
        let mut file = File::open(path).unwrap();
        let mut input_buffer: Vec<u8> = Vec::new();
        file.read_to_end(&mut input_buffer).unwrap();
        Self {
            input_buffer,
            output_file_name: ofn.into(),
        }
    }

    // Process a M3U8 path from a URL
    pub fn from_url<U: Into<String>>(_url: U, _output_file_name: U) -> Self {
        unimplemented!()
    }

    // Process straight from a byte array
    pub fn from_memory<U: Into<String>>(input_buffer: Vec<u8>, ofn: U) -> Self {
        Self {
            input_buffer,
            output_file_name: ofn.into(),
        }
    }

    /// Utils

    fn download_ts(uri: &String) -> Result<Vec<u8>, reqwest::Error> {
        let mut buffer: Vec<u8> = Vec::new();
        CLIENT.get(uri).send()?.read_to_end(&mut buffer).unwrap();
        Ok(buffer)
    }

    fn download_m3u8(uri: &String) -> Result<Vec<u8>, reqwest::Error> {
        let text = reqwest::get(uri)?.text()?;
        Ok(Vec::from(text.as_bytes()))
    }

    /// Processing

    fn process_master_playlist(&mut self, mp: &MasterPlaylist) {
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
        let m3u8_buffer = Self::download_m3u8(uris[selection]).unwrap();
        let mut m3u8 = M3U8::from_memory(m3u8_buffer, self.output_file_name.clone());
        m3u8.process();
    }

    fn process_media_playlist(&mut self, mp: &MediaPlaylist) {
        let pb = ProgressBar::new(mp.segments.len() as u64);
        let output_file = File::create(&self.output_file_name).unwrap();
        let mut writer = TsPacketWriter::new(output_file);
        for segment in &mp.segments {
            let buffer = Self::download_ts(&segment.uri);
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

    pub fn process(&mut self) {
        match m3u8_rs::parse_playlist_res(&self.input_buffer) {
            Ok(Playlist::MasterPlaylist(pl)) => self.process_master_playlist(&pl),
            Ok(Playlist::MediaPlaylist(pl)) => self.process_media_playlist(&pl),
            Err(e) => println!("Error: {:#?}", e),
        }
    }
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
        .arg(
            Arg::with_name("url")
                .short("u")
                .long("url")
                .value_name("URL")
                .help("Load a m3u8 file from a URL"),
        )
        .arg(
            Arg::with_name("path")
                .short("p")
                .long("path")
                .value_name("PATH")
                .default_value("output.ts")
                .help("Specify the output of the downloaded video"),
        )
        .get_matches();

    match matches.value_of("file") {
        Some(path) => {
            let mut m3u8 = M3U8::from_fs(path, matches.value_of("path").unwrap());
            m3u8.process();
        }
        None => {}
    }

    match matches.value_of("url") {
        Some(url) => {
            let mut m3u8 = M3U8::from_url(url, matches.value_of("path").unwrap());
            m3u8.process();
        }
        None => {}
    }
}
