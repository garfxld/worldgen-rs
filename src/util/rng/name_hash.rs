#[derive(Clone, Copy)]
pub struct NameHash {
    pub md5: [u64; 2],
    pub java_hash: i32,
}

impl NameHash {
    #[must_use]
    pub const fn new(name: &str) -> Self {
        let digest = const_md5(name.as_bytes());
        let lo = u64::from_be_bytes([
            digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
        ]);
        let hi = u64::from_be_bytes([
            digest[8], digest[9], digest[10], digest[11], digest[12], digest[13], digest[14],
            digest[15],
        ]);

        Self {
            md5: [lo, hi],
            java_hash: java_hash_code(name),
        }
    }
}

const fn java_hash_code(s: &str) -> i32 {
    let bytes = s.as_bytes();
    let mut hash = 0_i32;
    let mut i = 0;
    while i < bytes.len() {
        hash = hash.wrapping_mul(31).wrapping_add(bytes[i] as i32);
        i += 1;
    }
    hash
}

const S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

#[expect(
    clippy::unreadable_literal,
    reason = "RFC 1321 precomputed table; separators would obscure the standard values"
)]
const T: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

#[expect(
    clippy::many_single_char_names,
    reason = "matches RFC 1321 algorithm notation"
)]
#[expect(
    clippy::unreadable_literal,
    reason = "RFC 1321 initial hash values; underscores would obscure the standard constants"
)]
const fn const_md5(data: &[u8]) -> [u8; 16] {
    assert!(
        data.len() < 56,
        "const_md5: messages >= 56 bytes not supported"
    );

    let mut block = [0u8; 64];
    let mut i = 0;
    while i < data.len() {
        block[i] = data[i];
        i += 1;
    }
    block[data.len()] = 0x80;

    let bit_len = (data.len() as u64) * 8;
    let len_bytes = bit_len.to_le_bytes();
    i = 0;
    while i < 8 {
        block[56 + i] = len_bytes[i];
        i += 1;
    }

    let mut m = [0u32; 16];
    i = 0;
    while i < 16 {
        m[i] = u32::from_le_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
        i += 1;
    }

    let mut a: u32 = 0x67452301;
    let mut b: u32 = 0xefcdab89;
    let mut c: u32 = 0x98badcfe;
    let mut d: u32 = 0x10325476;

    i = 0;
    while i < 64 {
        let (f, g) = if i < 16 {
            ((b & c) | (!b & d), i)
        } else if i < 32 {
            ((d & b) | (!d & c), (5 * i + 1) % 16)
        } else if i < 48 {
            (b ^ c ^ d, (3 * i + 5) % 16)
        } else {
            (c ^ (b | !d), (7 * i) % 16)
        };

        let temp = d;
        d = c;
        c = b;
        let x = a.wrapping_add(f).wrapping_add(T[i]).wrapping_add(m[g]);
        b = b.wrapping_add(x.rotate_left(S[i]));
        a = temp;

        i += 1;
    }

    a = a.wrapping_add(0x67452301);
    b = b.wrapping_add(0xefcdab89);
    c = c.wrapping_add(0x98badcfe);
    d = d.wrapping_add(0x10325476);

    let ab = a.to_le_bytes();
    let bb = b.to_le_bytes();
    let cb = c.to_le_bytes();
    let db = d.to_le_bytes();
    [
        ab[0], ab[1], ab[2], ab[3], bb[0], bb[1], bb[2], bb[3], cb[0], cb[1], cb[2], cb[3], db[0],
        db[1], db[2], db[3],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn const_md5_matches_rfc_1321_test_vectors() {
        let cases: &[(&[u8], [u8; 16])] = &[
            (b"", hex_to_bytes("d41d8cd98f00b204e9800998ecf8427e")),
            (b"a", hex_to_bytes("0cc175b9c0f1b6a831c399e269772661")),
            (b"abc", hex_to_bytes("900150983cd24fb0d6963f7d28e17f72")),
            (
                b"message digest",
                hex_to_bytes("f96b697d7cb7938d525a2f31aaf161d0"),
            ),
            (
                b"abcdefghijklmnopqrstuvwxyz",
                hex_to_bytes("c3fcd3d76192e4007dfb496cca67e13b"),
            ),
        ];
        for &(input, expected) in cases {
            assert_eq!(const_md5(input), expected, "MD5 mismatch for {input:?}");
        }
    }

    #[test]
    fn java_hash_code_matches_utf16_definition() {
        for s in [
            "minecraft:offset",
            "minecraft:aquifer",
            "TEST STRING",
            "",
            "a",
        ] {
            let expected = s.encode_utf16().fold(0_i32, |hash, unit| {
                hash.wrapping_mul(31).wrapping_add(i32::from(unit))
            });
            assert_eq!(java_hash_code(s), expected, "hashCode mismatch for {s:?}");
        }
    }

    #[test]
    fn name_hash_is_evaluable_as_a_const() {
        const HASH: NameHash = NameHash::new("minecraft:offset");
        assert_ne!(HASH.md5, [0, 0]);
    }

    const fn hex_to_bytes(hex: &str) -> [u8; 16] {
        let bytes = hex.as_bytes();
        let mut out = [0u8; 16];
        let mut i = 0;
        while i < 16 {
            out[i] = hex_digit(bytes[i * 2]) * 16 + hex_digit(bytes[i * 2 + 1]);
            i += 1;
        }
        out
    }

    const fn hex_digit(c: u8) -> u8 {
        match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            _ => panic!("invalid hex digit"),
        }
    }
}
