use std::fmt::Write;
use std::ops::RangeInclusive;

pub fn url_encode_byte_string(data: Vec<u8>) -> String {
    let printable_range: RangeInclusive<u8> = 0x24..=0x7a;
    let unsafe_chars: [u8; 7] = [0x25, 0x60, 0x61, 0x62, 0x3c, 0x3d, 0x3e];
    let mut out = String::new();

    for val in data.iter() {
        if printable_range.contains(val) && !unsafe_chars.contains(val) {
            write!(&mut out, "{}", String::from_utf8(vec![*val]).unwrap()).unwrap();
        } else {
            write!(&mut out, "%{:X?}", val).unwrap();
        }
    }

    out
}

#[test]
fn test_url_encode_byte_string() {
    let source: [u8; 20] = [
        0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf1, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
        0x12, 0x34, 0x56, 0x78, 0x9a,
    ];

    let encoded = url_encode_byte_string(source.to_vec());

    assert_eq!(encoded, "%124Vx%9A%BC%DE%F1%23Eg%89%AB%CD%EF%124Vx%9A");
}
