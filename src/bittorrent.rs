use std::fmt::Display;

use bendy::decoding::FromBencode;
use rand::RngCore;
use sha1_checked::Sha1;

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
        let mut peer_id = Vec::new();
        rand::thread_rng().fill_bytes(&mut peer_id);
        PeerId(peer_id)
    }

    pub fn from_bytes(b: &[u8]) -> Self {
        PeerId(b.to_vec())
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
}
