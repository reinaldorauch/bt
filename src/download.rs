use bendy::decoding::FromBencode;
use reqwest::{Client, StatusCode, Url};
use std::{sync::Arc, time::Duration};
use tokio::{fs::File, sync::RwLock, task::JoinSet};

use crate::bittorrent::{
    AnnounceFailResult, DownloadProgress, PeerConnection, PeerInfoResult, TorrentError,
};

async fn announce(
    tracker: &String,
    info_hash: &crate::bittorrent::InfoHash,
    peer_id: &crate::bittorrent::PeerId,
    port: usize,
    progress_lock: &RwLock<DownloadProgress>,
) -> Result<PeerInfoResult, TorrentError> {
    let mut qs = vec![
        ("info_hash", info_hash.to_string()),
        ("peer_id", peer_id.to_string()),
        ("port", port.to_string()),
    ];

    println!("{:?}", qs);

    {
        let progress = progress_lock.read().await;

        if progress.bytes_downloaded > 0 {
            qs.push(("downloaded", progress.bytes_downloaded.to_string()));
            if progress.finished() {
                qs.push(("event", "finished".to_string()));
            }
        } else {
            qs.push(("event", "started".to_string()));
        }
        // dropping progress as then it can be released for other tasks
    }

    let client = Client::new();
    let url = Url::parse(tracker).map_err(|e| TorrentError::InvalidTrackerUrl(e.to_string()))?;

    match client.get(url.clone()).query(&qs).send().await {
        Ok(response) => {
            if response.status() != StatusCode::OK {
                return Err(TorrentError::TrackerError("Error response".into()));
            }

            let bytes = response
                .bytes()
                .await
                .map_err(|_| TorrentError::TrackerError("Unfinished response".into()))?;

            if let Ok(result) = AnnounceFailResult::from_bencode(bytes.to_vec().as_slice()) {
                return Err(TorrentError::TrackerError(result.to_string()));
            }

            PeerInfoResult::from_bytes(bytes.to_vec())
        }
        Err(e) => {
            println!("Error when announcing: {:?}", e);
            Err(TorrentError::TrackerError("Error in request".into()))
        }
    }
}

pub async fn download_files(
    maybe_trackers: Option<Vec<String>>,
    maybe_web_seeds: Option<Vec<String>>,
    info_hash: crate::bittorrent::InfoHash,
    peer_id: crate::bittorrent::PeerId,
    port: usize,
) -> () {
    let mut set = JoinSet::new();

    let download_progress: Arc<RwLock<DownloadProgress>> =
        Arc::new(RwLock::new(DownloadProgress::default()));

    let (tx, mut rx) = tokio::sync::mpsc::channel(64);

    if let Some(trackers) = maybe_trackers {
        println!(
            "Trying to download from these trackers: \n{}",
            trackers
                .iter()
                .map(|t| format!("    {}\n", t))
                .collect::<String>()
        );

        for t in trackers {
            let thread_info_hash = info_hash.clone();
            let thread_peer_id = peer_id.clone();
            let thread_download_progress = download_progress.clone();

            let thread_tx = tx.clone();

            set.spawn(async move {
                let _ = thread_tx
                    .send(format!("starting thread to announce the torrent"))
                    .await;

                let mut peers: Vec<PeerConnection> = Vec::new();
                let announce_interval = Duration::from_secs(60);

                loop {
                    match announce(
                        &t,
                        &thread_info_hash,
                        &thread_peer_id,
                        port,
                        &thread_download_progress,
                    )
                    .await
                    {
                        Ok(found_peers) => {
                            let _ = thread_tx
                                .send(format!("Got these peers {}", found_peers))
                                .await;
                            // peers.sort_by_key(|p| p.hostname.clone());
                            // for p in found_peers.peers {
                            //     let hostname = p.hostname();
                            //     if let Err(_) =
                            //         peers.binary_search_by_key(&hostname, |p| p.hostname.clone())
                            //     {
                            //         // Peer not found in current peer list, so make a connection to him
                            //         match PeerConnection::connect(
                            //             &hostname,
                            //             &thread_info_hash,
                            //             &thread_peer_id,
                            //         )
                            //         .await
                            //         {
                            //             Ok(c) => peers.push(c),
                            //             Err(e) => {
                            //                 println!("Could not connect to peer at {}: {}", hostname, e)
                            //             }
                            //         }
                            //     }
                            // }
                        }
                        Err(e) => {
                            let _ = thread_tx
                                .send(format!("Error when announcing: {}", e))
                                .await;
                        }
                    }

                    tokio::time::sleep(announce_interval).await;
                }
            });
        }
    } else {
        println!("this torrent doesnt have any defined tracker");
    }

    if let Some(web_seeds) = maybe_web_seeds {
        println!(
            "This torrent may download from these web seeds:\n{}",
            web_seeds
                .iter()
                .map(|ws| format!("    {}\n", ws))
                .collect::<String>()
        );
    } else {
        println!("this torrent doesnt have webseeds");
    }

    while let Some(msg) = rx.recv().await {
        println!("{}", msg);
    }

    set.join_all().await;

    ()
}

pub async fn download_single_file(
    pieces: Vec<String>,
    maybe_trackers: Option<Vec<String>>,
    maybe_web_seeds: Option<Vec<String>>,
    file_handle: &mut File,
) -> () {
    let mut pieces_downloaded: Vec<bool> = Vec::with_capacity(pieces.len());

    ()
}
