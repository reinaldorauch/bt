use std::fmt::Write;

use url_escape::encode_component_to_string;

pub fn url_encode_byte_string(data: Vec<u8>) -> String {
    let mut buffer = String::new();

    for c in data {
        if c < 32 || c >= 127 {
            buffer.write_str(format!("%{:02x}", c).as_str()).unwrap();
        } else {
            let nc: char = c.into();
            buffer.write_char(nc).unwrap();
        }
    }

    buffer
}

#[test]
fn test_encode_byte_string() {
    let bytes: [u8; 20] = [
        0x42, 0x52, 0x5b, 0xb6, 0xd3, 0xb0, 0xdc, 0x06, 0xbb, 0x78, 0xae, 0x54, 0x87, 0x33, 0xe8,
        0xfb, 0xb5, 0x54, 0x46, 0xb3,
    ];
    let dest = url_encode_byte_string(bytes.to_vec());
    let mut component = String::new();
    encode_component_to_string(dest, &mut component);

    assert_eq!(
        component,
        "BR%5B%EF%BF%BD%D3%B0%EF%BF%BD%06%EF%BF%BDx%EF%BF%BDT%EF%BF%BD3%EF%BF%BD%EF%BF%BD%EF%BF%BDTF%EF%BF%BD"
    );
}
