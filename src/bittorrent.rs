use std::{
    fmt::{Display, write},
    str::FromStr,
};

use bendy::decoding::{FromBencode, Object};
use rand::RngCore;
use reqwest::Url;
use sha1_checked::Sha1;
use tokio::{io::AsyncWriteExt, net::TcpStream};

use crate::util::url_encode_byte_string;

#[derive(Debug, PartialEq, Clone)]
pub struct PeerId(Vec<u8>);

impl Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", url_encode_byte_string(self.0.clone()))
    }
}

impl PeerId {
    pub fn new() -> Self {
        let mut peer_id: Vec<u8> = [b'-', b'L', b'T', b'0', b'0', b'1', b'0', b'-'].to_vec();
        let mut rand_peer_id: [u8; 12] = [0; 12];
        rand::thread_rng().fill_bytes(&mut rand_peer_id);

        peer_id.append(&mut rand_peer_id.to_vec());

        PeerId(peer_id)
    }

    pub fn from_bytes(b: &[u8]) -> Self {
        PeerId(b.to_vec())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0.as_slice()
    }
}

impl FromBencode for PeerId {
    fn decode_bencode_object(
        object: bendy::decoding::Object,
    ) -> Result<Self, bendy::decoding::Error>
    where
        Self: Sized,
    {
        let bytes = object.try_into_bytes()?;
        Ok(PeerId::from_bytes(bytes))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct InfoHash(Vec<u8>);

impl Display for InfoHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", url_encode_byte_string(self.0.clone()))
    }
}

impl InfoHash {
    pub fn from_info_bytes(info_bytes: &[u8]) -> Self {
        InfoHash(Sha1::try_digest(info_bytes).hash().to_vec())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0.as_slice()
    }
}

pub enum PeerConnectionError {
    InvalidUrl(String),
    SocketUnavailable(String),
    Other(String),
}

impl Display for PeerConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use PeerConnectionError::*;

        match self {
            InvalidUrl(e) => write!(f, "PeerConnectionError::InvalidUrl: {}", e),
            SocketUnavailable(e) => write!(f, "PeerConnectionError::SocketUnavailable: {}", e),
            Other(e) => write!(f, "PeerConnectionError::Other: {}", e),
        }
    }
}

#[derive(Debug)]
pub struct PeerConnection {
    pub hostname: String,
    socket: TcpStream,
    pub me_choked: bool,
    pub me_interested: bool,
    pub they_choked: bool,
    pub they_interested: bool,
}

impl PeerConnection {
    pub async fn connect(
        url: &String,
        info_hash: &InfoHash,
        peer_id: &PeerId,
    ) -> Result<Self, PeerConnectionError> {
        let mut conn = PeerConnection {
            hostname: Url::from_str(url.as_str())
                .map_err(|e| PeerConnectionError::InvalidUrl(e.to_string()))?
                .host()
                .ok_or_else(|| PeerConnectionError::InvalidUrl("has no hostname".to_string()))?
                .to_string(),
            socket: TcpStream::connect(url)
                .await
                .map_err(|err| PeerConnectionError::Other(err.to_string()))?,
            me_choked: true,
            me_interested: false,
            they_choked: true,
            they_interested: false,
        };

        conn.handshake(&info_hash, &peer_id).await?;

        Ok(conn)
    }

    pub async fn handshake(
        &mut self,
        info_hash: &InfoHash,
        peer_id: &PeerId,
    ) -> Result<(), PeerConnectionError> {
        let mut buffer: Vec<u8> = Vec::new();
        std::io::Write::write(&mut buffer, &[0x13]).unwrap();
        std::io::Write::write(&mut buffer, b"BitTorrent protocol" as &[u8]).unwrap();
        std::io::Write::write(&mut buffer, info_hash.as_bytes()).unwrap();
        std::io::Write::write(&mut buffer, peer_id.as_bytes()).unwrap();

        self.socket
            .write_all(buffer.as_slice())
            .await
            .map_err(|e| PeerConnectionError::SocketUnavailable(e.to_string()))?;

        self.socket
            .flush()
            .await
            .map_err(|e| PeerConnectionError::SocketUnavailable(e.to_string()))?;
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct DownloadProgress {
    pub bytes_total: u64,
    pub bytes_downloaded: u64,
    pub bytes_uploaded: u64,
    pub pieces_fetched: Vec<bool>,
}

impl DownloadProgress {
    pub fn finished(&self) -> bool {
        self.bytes_downloaded == self.bytes_total
    }
}

#[derive(Debug, PartialEq)]
pub struct Peer {
    pub id: Option<PeerId>,
    pub ip: String,
    pub port: usize,
}

impl Peer {
    pub fn hostname(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }
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
                (b"id", val) => id = Some(PeerId::decode_bencode_object(val)?),
                (b"ip", val) => ip = Some(String::decode_bencode_object(val)?),
                (b"port", val) => port = Some(usize::decode_bencode_object(val)?),
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
pub struct PeerInfoResult {
    warning_message: Option<String>,
    interval: u64,
    min_interval: Option<u64>,
    tracker_id: Option<String>,
    complete: u64,
    incomplete: u64,
    peers: Vec<Peer>,
}

#[derive(Debug, PartialEq)]
pub struct AnnounceFailResult {
    failure_reason: String,
}

impl Into<String> for AnnounceFailResult {
    fn into(self) -> String {
        format!("AnnounceFailResult: {}", self.failure_reason)
    }
}

impl Display for AnnounceFailResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.failure_reason)
    }
}

impl FromBencode for AnnounceFailResult {
    fn decode_bencode_object(object: Object) -> Result<Self, bendy::decoding::Error>
    where
        Self: Sized,
    {
        let mut dict = object.try_into_dictionary()?;

        let mut maybe_failure_reason = None;

        while let Some(pair) = dict.next_pair()? {
            match pair {
                (b"failure reason", val) => {
                    maybe_failure_reason = Some(String::decode_bencode_object(val)?);
                }
                (_, _) => {}
            }
        }

        if let Some(failure_reason) = maybe_failure_reason {
            Ok(AnnounceFailResult { failure_reason })
        } else {
            Err(bendy::decoding::Error::missing_field("failure reason"))
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct AnnounceError(String);

impl Display for PeerInfoResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Peer info result:\n")?;

        if let Some(wm) = &self.warning_message {
            write!(f, "Warning message: {}\n", wm)?;
        }

        if let Some(mi) = &self.min_interval {
            write!(f, "min interval: {}\n", mi)?;
        }

        if let Some(ti) = &self.tracker_id {
            write!(f, "tracker id: {}\n", ti)?
        }

        write!(
            f,
            "    interval: {}\n    complete: {}\n    incomplete: {}\n    peers: {}",
            &self.interval,
            &self.complete,
            &self.incomplete,
            &self
                .peers
                .iter()
                .map(|p| format!("    {}", p))
                .collect::<String>()
        )
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
                (b"peers", val) => {
                    let peer_bytes = val.try_into_bytes()?;

                    let mut peer_list: Vec<Peer> = vec![];
                    if let Ok(mut list) = Object::Bytes(peer_bytes).try_into_list() {
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
                (b"tracker_id", val) => tracker_id = Some(String::decode_bencode_object(val)?),
                (b"complete", val) => complete = Some(u64::decode_bencode_object(val)?),
                (b"incomplete", val) => incomplete = Some(u64::decode_bencode_object(val)?),
                (b"interval", val) => interval = Some(u64::decode_bencode_object(val)?),
                (b"min interval", val) => min_interval = Some(u64::decode_bencode_object(val)?),
                (b"warning message", val) => {
                    warning_message = Some(String::decode_bencode_object(val)?)
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

        Ok(PeerInfoResult {
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
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, TorrentError> {
        PeerInfoResult::from_bencode(bytes.as_slice())
            .map_err(|e| TorrentError::InvalidAnnounceResponse(e.to_string()))
    }
}

#[derive(Debug)]
pub enum TorrentError {
    TrackerError(String),
    InvalidAnnounceResponse(String),
    InvalidTrackerUrl(String),
}

impl Into<String> for TorrentError {
    fn into(self) -> String {
        self.to_string()
    }
}

impl Display for TorrentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use TorrentError::*;

        match self {
            TrackerError(e) => write!(f, "TrackerError: {}", e),
            InvalidAnnounceResponse(e) => write!(f, "AnnounceError: {}", e),
            InvalidTrackerUrl(e) => write!(f, "InvalidTrackerUrl: {}", e),
        }
    }
}
