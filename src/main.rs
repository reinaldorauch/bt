#![feature(iter_intersperse)]

mod bittorrent;
mod download;
mod metainfo;
mod util;

use bendy::decoding::FromBencode;
use chrono::DateTime;
use clap::Parser;
use download::{download_files, download_single_file};
use metainfo::MetaInfoFile;
use std::env;
use tokio::fs::OpenOptions;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct CliOptions {
    /// Torrent file to donwload
    torrent_file_path: std::path::PathBuf,

    /// Show, parsed metadata from file
    #[arg(short, long)]
    verbose: bool,

    /// Sets the download dir. Defaults to $PWD
    #[arg(short, long, value_name = "DIR")]
    download_dir: Option<std::path::PathBuf>,
}

#[tokio::main]
async fn main() {
    let args = CliOptions::parse();

    // @TODO: persist data to disk
    let peer_id = bittorrent::PeerId::new();
    let bt_listen_port = 6881usize;

    println!("File path: {:?}", args.torrent_file_path);

    let torrent_file = std::fs::read(args.torrent_file_path).expect("Could not read torrent file.");

    let meta =
        MetaInfoFile::from_bencode(&torrent_file).expect("Error parsing bencode metainfo file");

    println!(
        "Announces:\nannounce: {:?}\nannouce-list: {:?}",
        meta.announce, meta.announce_list
    );

    if let Some(d) = meta.creation_date {
        println!(
            "creation date: {}",
            DateTime::from_timestamp(d.try_into().unwrap(), 0)
                .unwrap()
                .format("%Y-%m-%d %H:%M:%S")
        )
    }

    if let Some(c) = meta.comment {
        println!("Comment: {}", c);
    }

    if let Some(cb) = meta.created_by {
        println!("created by: {}", cb);
    }

    if let Some(e) = meta.encoding {
        println!("encoding: {}", e);
    }

    if args.verbose {
        println!("Info:\n{}", meta.info);
    }

    let download_dir = args
        .download_dir
        .or_else(|| env::current_dir().map(Some).expect("could not get pwd"))
        .expect("could not get download dir");

    // Allocate files:

    match meta.info {
        metainfo::Info::SingleFileInfo {
            name,
            piece_length,
            pieces,
            private,
            length,
        } => {
            let mut file_handle = OpenOptions::new()
                .write(true)
                .create(true)
                .open(&name)
                .await
                .expect(format!("could not open {} file", &name).as_str());

            let trackers = if let Some(tr) = meta.announce {
                let mut list = vec![tr];

                if let Some(trlist) = meta.announce_list {
                    list.append(&mut trlist.clone());
                }

                Some(list)
            } else {
                None
            };

            let web_seeds = if let Some(ws) = meta.url_list {
                Some(ws)
            } else {
                None
            };

            download_single_file(pieces, trackers, web_seeds, &mut file_handle).await
        }
        metainfo::Info::MultiFileInfo {
            name,
            piece_length,
            pieces,
            private,
            files,
        } => {
            let torrent_dir_path = download_dir.join(name);

            let torrent_dir_exists = std::fs::exists(&torrent_dir_path)
                .expect("could not check if torrent directory exists");
            if !torrent_dir_exists {
                std::fs::create_dir(&torrent_dir_path).expect("could not create main dir");
            }

            let trackers: Option<Vec<String>>;

            if let Some(announce) = meta.announce {
                let mut ts = vec![announce];
                if let Some(list) = meta.announce_list {
                    ts.append(&mut list.clone());
                }

                trackers = Some(ts);
            } else {
                trackers = None;
                println!("No trackers to download");
            }

            let web_seeds: Option<Vec<String>>;

            if let Some(ws) = meta.url_list {
                web_seeds = Some(ws);
            } else {
                web_seeds = None;
            }

            download_files(trackers, web_seeds, meta.info_hash, peer_id, bt_listen_port).await
        }
    }
}
