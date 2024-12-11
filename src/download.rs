use bendy::decoding::{FromBencode, Object, ResultExt};
use reqwest::{Client, StatusCode};
use std::{fmt::Display, time::Duration};

use crate::bittorrent::PeerId;

#[derive(Debug, PartialEq)]
pub struct Peer {
    pub id: Option<PeerId>,
    pub ip: String,
    pub port: usize,
}

impl Peer {
    pub fn from_slice(b: &[u8]) -> Self {
        let ip = b[0..=4]
            .iter()
            .map(|n| char::from(*n))
            .intersperse('.')
            .collect::<String>();

        let mut buffer: [u8; 8] = [0; 8];

        buffer.copy_from_slice(&b[5..]);

        Peer {
            id: None,
            ip,
            port: usize::from_be_bytes(buffer),
        }
    }
}

impl Display for Peer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(id) = &self.id {
            write!(f, "    [{}] {}:{}", id, self.ip, self.port)
        } else {
            write!(f, "    [no id] {}:{}", self.ip, self.port)
        }
    }
}

#[test]
fn test_peer_from_slice() {
    let slice: [u8; 6] = [10, 0, 0, 1, 0x1a, 0xe1];

    assert_eq!(
        Peer {
            id: None,
            ip: "10.0.0.1".to_string(),
            port: 6881
        },
        Peer::from_slice(&slice)
    )
}

impl FromBencode for Peer {
    const EXPECTED_RECURSION_DEPTH: usize = 1;

    fn decode_bencode_object(object: Object) -> Result<Self, bendy::decoding::Error>
    where
        Self: Sized,
    {
        let mut decoder = object.try_into_dictionary()?;

        let mut id = None;
        let mut ip = None;
        let mut port = None;

        while let Some(pair) = decoder.next_pair()? {
            match pair {
                (b"id", val) => id = Some(PeerId::decode_bencode_object(val).context("id")?),
                (b"ip", val) => ip = Some(String::decode_bencode_object(val).context("ip")?),
                (b"port", val) => port = Some(usize::decode_bencode_object(val).context("port")?),
                (f, _) => {
                    let field = String::from_utf8(f.to_vec()).expect("malformed key value");
                    return Err(bendy::decoding::Error::unexpected_field(field));
                }
            }
        }

        if let (None, None) = (ip.clone(), port) {
            return Err(bendy::decoding::Error::missing_field("ip or port not set"));
        }

        Ok(Peer {
            id,
            ip: ip.expect("should have set ip"),
            port: port.expect("should have set port"),
        })
    }
}

#[derive(Debug, PartialEq)]
pub enum PeerInfoResult {
    Error(String),
    PeerInfo {
        warning_message: Option<String>,
        interval: u64,
        min_interval: Option<u64>,
        tracker_id: Option<String>,
        complete: u64,
        incomplete: u64,
        peers: Vec<Peer>,
    },
}

impl Display for PeerInfoResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use PeerInfoResult::*;

        match self {
            Error(s) => {
                write!(f, "Error result: {}", s)
            }
            PeerInfo {
                warning_message,
                interval,
                min_interval,
                tracker_id,
                complete,
                incomplete,
                peers,
            } => {
                write!(f, "Peer info result:\n")?;

                if let Some(wm) = warning_message {
                    write!(f, "Warning message: {}\n", wm)?;
                }

                if let Some(mi) = min_interval {
                    write!(f, "min interval: {}\n", mi)?;
                }

                if let Some(ti) = tracker_id {
                    write!(f, "tracker id: {}\n", ti)?
                }

                write!(
                    f,
                    "    interval: {}\n    complete: {}\n    incomplete: {}\n    peers: {}",
                    interval,
                    complete,
                    incomplete,
                    peers
                        .iter()
                        .map(|p| format!("    {}", p))
                        .collect::<String>()
                )
            }
        }
    }
}

impl FromBencode for PeerInfoResult {
    fn decode_bencode_object(object: Object) -> Result<Self, bendy::decoding::Error>
    where
        Self: Sized,
    {
        let mut decoder = object.try_into_dictionary()?;

        let mut peers = None;
        let mut tracker_id = None;
        let mut complete = None;
        let mut incomplete = None;
        let mut interval = None;
        let mut min_interval = None;
        let mut warning_message = None;

        while let Some(pair) = decoder.next_pair()? {
            match pair {
                (b"failure reason", val) => {
                    let f = String::decode_bencode_object(val)?;
                    return Ok(PeerInfoResult::Error(f));
                }
                (b"peers", val) => {
                    let peer_bytes = val.try_into_bytes().context("peers")?;

                    let mut peer_list: Vec<Peer> = vec![];
                    if let Ok(mut list) = Object::Bytes(peer_bytes).try_into_list().context("peers")
                    {
                        while let Some(val) = list.next_object()? {
                            peer_list.push(Peer::decode_bencode_object(val)?);
                        }
                    } else {
                        for peer in peer_bytes.chunks(6) {
                            peer_list.push(Peer::from_slice(peer));
                        }
                    }
                    peers = Some(peer_list);
                }
                (b"tracker_id", val) => {
                    tracker_id = Some(String::decode_bencode_object(val).context("tracker_id")?)
                }
                (b"complete", val) => {
                    complete = Some(u64::decode_bencode_object(val).context("complete")?)
                }
                (b"incomplete", val) => {
                    incomplete = Some(u64::decode_bencode_object(val).context("incomplete")?)
                }
                (b"interval", val) => {
                    interval = Some(u64::decode_bencode_object(val).context("interval")?)
                }
                (b"min interval", val) => {
                    min_interval = Some(u64::decode_bencode_object(val).context("min interval")?)
                }
                (b"warning message", val) => {
                    warning_message =
                        Some(String::decode_bencode_object(val).context("warning message")?)
                }
                (f, _) => {
                    let field = String::from_utf8(f.to_vec()).expect("malformed key value");
                    return Err(bendy::decoding::Error::unexpected_field(field));
                }
            }
        }
        if let (None, None, None) = (interval, complete, incomplete) {
            if let None = interval {
                return Err(bendy::decoding::Error::missing_field("interval"));
            }

            if let None = complete {
                return Err(bendy::decoding::Error::missing_field("complete"));
            }

            if let None = incomplete {
                return Err(bendy::decoding::Error::missing_field("incomplete"));
            }
        }

        Ok(PeerInfoResult::PeerInfo {
            warning_message,
            interval: interval.expect("should contain interval"),
            min_interval,
            tracker_id,
            complete: complete.expect("should contain complete"),
            incomplete: incomplete.expect("should contain incomplete"),
            peers: peers.expect("should contain peers"),
        })
    }
}

impl PeerInfoResult {
    fn from_bytes(bytes: Vec<u8>) -> Result<Self, TorrentError> {
        let obj = Object::Bytes(bytes.as_slice());

        PeerInfoResult::decode_bencode_object(obj)
            .map_err(|e| TorrentError::InvalidAnnounceResponse(e.to_string()))
    }
}

#[derive(Debug)]
pub enum TorrentError {
    TrackerError(String),
    InvalidAnnounceResponse(String),
}

async fn announce(
    tracker: &String,
    info_hash: crate::bittorrent::InfoHash,
    peer_id: crate::bittorrent::PeerId,
    port: usize,
) -> Result<PeerInfoResult, TorrentError> {
    let qs = vec![
        ("info_hash", info_hash.to_string()),
        ("peer_id", peer_id.to_string()),
        ("port", port.to_string()),
    ];
    let client = Client::new();

    match client.get(tracker).query(&qs).send().await {
        Ok(response) => {
            if response.status() != StatusCode::OK {
                return Err(TorrentError::TrackerError("Error response".into()));
            }

            let bytes = response
                .bytes()
                .await
                .map_err(|_| TorrentError::TrackerError("Unfinished response".into()))?;

            PeerInfoResult::from_bytes(bytes.to_vec())
        }
        Err(e) => {
            println!("Error when announcing: {:?}", e);
            Err(TorrentError::TrackerError("Error in request".into()))
        }
    }
}

pub async fn download_files(
    trackers: Vec<String>,
    info_hash: crate::bittorrent::InfoHash,
    peer_id: crate::bittorrent::PeerId,
    bt_listen_port: usize,
) {
    // Announcing

    tokio::task::spawn(async move {
        loop {
            for t in trackers.iter() {
                let peer_info =
                    announce(t, info_hash.clone(), peer_id.clone(), bt_listen_port).await;

                if let Err(e) = peer_info {
                    use TorrentError::*;
                    match e {
                        InvalidAnnounceResponse(e) => {
                            println!("Invalid announce response: {}", e)
                        }
                        TrackerError(e) => println!("tracker error: {}", e),
                    };
                } else {
                    println!("got peer info: {}", peer_info.unwrap());
                }
            }

            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}
