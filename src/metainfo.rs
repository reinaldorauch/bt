use std::{fmt::Display, vec};

use bendy::decoding::{FromBencode, Object, ResultExt};

use crate::bittorrent::InfoHash;

#[derive(PartialEq, Debug)]
pub struct File {
    length: u64,
    path: Vec<String>,
    md5sum: Option<String>,
}

impl FromBencode for File {
    fn decode_bencode_object(
        object: bendy::decoding::Object,
    ) -> Result<Self, bendy::decoding::Error>
    where
        Self: Sized,
    {
        let mut path = None;
        let mut length = None;
        let mut md5sum = None;

        let mut dict = object
            .try_into_dictionary()
            .expect("Shoudl be a dictionary");

        while let Some(pair) = dict.next_pair().expect("File should have pairs") {
            match pair {
                (b"length", l) => {
                    length = u64::decode_bencode_object(l).context("length").map(Some)?;
                }
                (b"path", p) => {
                    let mut list = p.try_into_list()?;
                    let mut path_list = vec![];

                    while let Some(list_item) = list.next_object()? {
                        path_list.push(String::decode_bencode_object(list_item)?);
                    }

                    path = Some(path_list);
                }
                (b"md5sum", h) => {
                    md5sum = String::decode_bencode_object(h)
                        .context("md5sum")
                        .map(Some)?;
                }
                (_, _) => {}
            }
        }

        if length == None || path == None {
            panic!("no length or path");
        }

        Ok(File {
            length: length.unwrap(),
            path: path.unwrap(),
            md5sum,
        })
    }
}

impl Display for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Name: {}, size: {}, md5sum: {:?}",
            self.path.join(" - "),
            self.length,
            self.md5sum
        )
    }
}

#[derive(PartialEq, Debug)]

pub enum Info {
    SingleFileInfo {
        name: String,
        piece_length: u64,
        pieces: Vec<String>,
        length: u64,
        private: Option<bool>,
    },
    MultiFileInfo {
        name: String,
        piece_length: u64,
        pieces: Vec<String>,
        private: Option<bool>,
        files: Vec<File>,
    },
}

impl FromBencode for Info {
    fn decode_bencode_object(
        object: bendy::decoding::Object,
    ) -> Result<Self, bendy::decoding::Error> {
        let mut dict = object
            .try_into_dictionary()
            .expect("Info must be a dictionary");

        let mut name = None;
        let mut piece_length = None;
        let mut pieces = None;
        let mut length = None;
        let mut private = None;
        let mut files = None;

        while let Some(pair) = dict.next_pair()? {
            match pair {
                (b"name", val) => {
                    name = String::decode_bencode_object(val)
                        .context("mame")
                        .map(Some)?
                }
                (b"piece length", val) => {
                    piece_length = u64::decode_bencode_object(val)
                        .context("piece lenth")
                        .map(Some)?
                }
                (b"pieces", val) => {
                    let raw_pieces: Vec<String> = val
                        .try_into_bytes()
                        .expect("could not parse pieces key")
                        .chunks(20)
                        .map(|c| hex::encode(c))
                        .collect();

                    pieces = Some(raw_pieces);
                }
                (b"length", val) => {
                    length = u64::decode_bencode_object(val)
                        .context("length")
                        .map(Some)?
                }
                (b"private", val) => {
                    let private_val = u8::decode_bencode_object(val).context("private")?;

                    private = Some(private_val == 1);
                }
                (b"files", val) => {
                    let mut list = val.try_into_list().expect("files must be a list");
                    let mut file_list: Vec<File> = vec![];

                    while let Some(item) = list.next_object()? {
                        file_list.push(File::decode_bencode_object(item).context("files")?);
                    }

                    files = Some(file_list);
                }
                (_, _) => {}
            }
        }

        if let Some(_) = length {
            Ok(Info::SingleFileInfo {
                name: name.expect("should have name key"),
                piece_length: piece_length.expect("should have piece length key"),
                pieces: pieces.expect("should have pieces key"),
                length: length.expect("should have length key"),
                private,
            })
        } else {
            Ok(Info::MultiFileInfo {
                name: name.expect("should have name key"),
                piece_length: piece_length.expect("should have piece length key"),
                pieces: pieces.expect("should have pieces key"),
                files: files.expect("should have files key"),
                private,
            })
        }
    }
}

impl Display for Info {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Info::SingleFileInfo {
                name,
                piece_length,
                pieces,
                length,
                private,
            } => {
                write!(
                    f,
                    "Name: {}\npiece length: {}\npieces: {}\n Single file length: {}\nprivate? {}",
                    name,
                    piece_length,
                    pieces.len(),
                    length,
                    if let Some(v) = private {
                        if *v {
                            "yes"
                        } else {
                            "no"
                        }
                    } else {
                        "no"
                    }
                )
            }
            Info::MultiFileInfo {
                name,
                piece_length,
                pieces,
                private,
                files,
            } => {
                write!(
                    f,
                    "Name: {}\npiece length: {}\npieces: {}\nprivate? {}\nMultiple files:\n{}",
                    name,
                    piece_length,
                    pieces.len(),
                    if let Some(v) = private {
                        if *v {
                            "yes"
                        } else {
                            "no"
                        }
                    } else {
                        "no"
                    },
                    files.iter().map(|f| format!("{}\n", f)).collect::<String>()
                )
            }
        }
    }
}

#[derive(PartialEq, Debug)]
pub struct MetaInfoFile {
    pub announce: String,
    pub announce_list: Option<Vec<String>>,
    pub info: Info,
    pub created_by: Option<String>,
    pub creation_date: Option<u64>,
    pub comment: Option<String>,
    pub encoding: Option<String>,
    pub info_hash: InfoHash,
}

impl FromBencode for MetaInfoFile {
    fn decode_bencode_object(
        object: bendy::decoding::Object,
    ) -> Result<Self, bendy::decoding::Error> {
        let mut dict = object
            .try_into_dictionary()
            .expect("meta file must be a dict");

        let mut announce = None;
        let mut announce_list = None;
        let mut created_by = None;
        let mut info: Option<Info> = None;
        let mut comment = None;
        let mut creation_date = None;
        let mut encoding = None;
        let mut info_hash = None;

        while let Some(pair) = dict.next_pair()? {
            match pair {
                (b"announce", val) => {
                    announce = String::decode_bencode_object(val)
                        .context("announce")
                        .map(Some)?
                }
                (b"announce-list", val) => {
                    if let Ok(mut list) = val.try_into_list() {
                        let mut announce_vec: Vec<String> = vec![];

                        while let Some(o1) = list.next_object()? {
                            let mut l2 = o1.try_into_list()?;
                            while let Some(o2) = l2.next_object()? {
                                announce_vec.push(
                                    String::decode_bencode_object(o2).context("announce-list")?,
                                );
                            }
                        }

                        announce_list = Some(announce_vec);
                    }
                }
                (b"created by", val) => {
                    created_by = String::decode_bencode_object(val)
                        .context("created by")
                        .map(Some)?
                }
                (b"info", val) => {
                    let info_bytes = val.try_into_bytes().context("info")?;

                    info_hash = Some(InfoHash::from_info_bytes(info_bytes));

                    info = Info::decode_bencode_object(Object::Bytes(info_bytes))
                        .context("single file info")
                        .map(Some)?;
                }
                (b"comment", val) => {
                    comment = String::decode_bencode_object(val)
                        .context("comment")
                        .map(Some)?
                }
                (b"creation date", val) => {
                    creation_date = u64::decode_bencode_object(val)
                        .context("creation date")
                        .map(Some)?
                }
                (b"encoding", val) => {
                    encoding = String::decode_bencode_object(val)
                        .context("encoding")
                        .map(Some)?
                }
                (_, _) => {}
            }
        }

        Ok(MetaInfoFile {
            announce: announce.expect("must have announce key"),
            announce_list,
            created_by,
            info: info.expect("Must have info key"),
            comment,
            creation_date,
            encoding,
            info_hash: info_hash.expect("should have info hash"),
        })
    }
}
