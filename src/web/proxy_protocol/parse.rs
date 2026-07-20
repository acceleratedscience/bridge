use std::net::{Ipv4Addr, SocketAddr};

/*
    PPV2 Header Parsing Breakdown

    16 Byte Structure:
    1. Signature: 12 bytes
    2. Version and Command: 1 byte
        - Version: 4 bits (high nibble)
        - Command: 4 bits (low nibble)
    3. Address Family and Protocol: 1 byte
        - Address Family: 4 bits (high nibble)
        - Protocol: 4 bits (low nibble)
    4. Length: 2 bytes (big-endian)
    5. Address Information: variable length based on the length field
        - Based on 3, the address information can be:
            - IPv4: 12 bytes (4 bytes source IP, 4 bytes destination IP, 2 bytes source port, 2 bytes destination port)
            - IPv6: 36 bytes (16 bytes source IP, 16 bytes destination IP, 2 bytes source port, 2 bytes destination port)
            - UNIX: 216 bytes (108 bytes source path, 108 bytes destination path)
    6. The total length of the header is 16 bytes + the length of the address information.
*/

pub const SIGNATURE: [u8; 12] = [
    0x0D, 0x0A, 0x0D, 0x0A, 0x00, 0x0D, 0x0A, 0x51, 0x55, 0x49, 0x54, 0x0A,
];

#[derive(Debug, PartialEq)]
pub enum Parsed {
    Proxied(SocketAddr),
    LocalOrUnspec,
    NotProxy,
}

/// Parses a buffer containing a Proxy Protocol v2 header and returns a `Parsed` enum indicating the result.
pub fn parse_pp_v2(buffer: &[u8]) -> Parsed {
    if buffer.len() < 16 || buffer[..12] != SIGNATURE {
        return Parsed::NotProxy;
    }

    let version = buffer[12] >> 4;
    if version != 2 {
        return Parsed::NotProxy;
    }

    let command = buffer[12] & 0x0F;
    if command == 0x0 {
        // Do not parse address if the command is LOCAL / 0x0
        return Parsed::LocalOrUnspec;
    }

    let address_family = buffer[13] >> 4;
    let protocol = buffer[13] & 0x0F;
    if address_family == 0x0 || protocol == 0x0 {
        return Parsed::LocalOrUnspec;
    }

    let address_length = u16::from_be_bytes([buffer[14], buffer[15]]) as usize;
    let address = &buffer[16..16 + address_length];
    match address_family {
        0x1 if address.len() >= 12 => {
            // IPv4
            let ip = Ipv4Addr::new(address[0], address[1], address[2], address[3]);
            let port = u16::from_be_bytes([address[8], address[9]]);
            Parsed::Proxied(SocketAddr::new(ip.into(), port))
        }
        0x2 if address.len() >= 36 => {
            // IPv6
            let mut o = [0u8; 16];
            o.copy_from_slice(&address[0..16]);
            let port = u16::from_be_bytes([address[32], address[33]]);
            Parsed::Proxied(SocketAddr::new(o.into(), port))
        }
        _ => {
            // Unsupported address family
            // Not supporting Unix socket 0x3 for now, return LocalOrUnspec
            Parsed::LocalOrUnspec
        }
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_ipv4() {
        use super::*;
        let buffer: Vec<u8> = vec![
            0x0D, 0x0A, 0x0D, 0x0A, 0x00, 0x0D, 0x0A, 0x51, 0x55, 0x49, 0x54, 0x0A,
            0x21, // Version and Command (2 and PROXY)
            0x11, // Address Family and Protocol (IPv4 and TCP)
            0x00, 0x0C, // Length (12 bytes for IPv4)
            192, 168, 1, 1, // Source IP
            192, 168, 1, 2, // Destination IP
            0x1F, 0x90, // Source Port (8080)
            0x00, 0x50, // Destination Port (80)
        ];

        let parsed = parse_pp_v2(&buffer);
        assert_eq!(
            parsed,
            Parsed::Proxied(SocketAddr::new(Ipv4Addr::new(192, 168, 1, 1).into(), 8080))
        );
    }

    #[test]
    fn test_ipv6() {
        use super::*;
        let buffer: Vec<u8> = vec![
            0x0D, 0x0A, 0x0D, 0x0A, 0x00, 0x0D, 0x0A, 0x51, 0x55, 0x49, 0x54, 0x0A,
            0x21, // Version and Command (2 and PROXY)
            0x21, // Address Family and Protocol (IPv6 and TCP)
            0x00, 0x24, // Length (36 bytes for IPv6)
            // Source IP (16 bytes)
            32, 1, 13, 184, 133, 163, 0, 1, 32, 1, 13, 184, 133, 163, 0, 2,
            // Destination IP (16 bytes)
            32, 1, 13, 184, 133, 163, 0, 3, 32, 1, 13, 184, 133, 163, 0, 4,
            // Source Port (2 bytes)
            0x1F, 0x90, // Destination Port (2 bytes)
            0x00, 0x50,
        ];

        let parsed = parse_pp_v2(&buffer);
        assert_eq!(
            parsed,
            Parsed::Proxied(SocketAddr::new(
                "2001:db8:85a3:1:2001:db8:85a3:2".parse().unwrap(),
                8080
            ))
        );
    }
}
