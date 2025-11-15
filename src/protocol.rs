// src/protocol.rs
use crate::UotError;
use bytes::{Buf, BufMut, BytesMut};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

pub const VERSION: u8 = 2;
pub const LEGACY_VERSION: u8 = 1;
pub const MAGIC_ADDRESS: &str = "sp.v2.udp-over-tcp.arpa";
pub const LEGACY_MAGIC_ADDRESS: &str = "sp.udp-over-tcp.arpa";

// A SOCKS-style address, similar to M.Socksaddr
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SocksAddr {
    Ip(SocketAddr),
    Domain(String, u16),
}

// The UoT Request, from protocol.go
#[derive(Debug, Clone)]
pub struct UotRequest {
    pub is_connect: bool,
    pub destination: SocksAddr,
}

impl UotRequest {
    // Translates EncodeRequest
    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), UotError> {
        // Write `is_connect` (1 byte)
        buf.put_u8(self.is_connect as u8);
        // Write `destination` (SOCKS-style)
        self.destination.encode(buf)?;
        Ok(())
    }

    // Translates ReadRequest
    pub async fn decode(buf: &mut impl Buf) -> Result<Self, UotError> {
        if buf.remaining() < 1 {
            return Err(UotError::Protocol("Buffer too short for IsConnect".into()));
        }
        let is_connect = buf.get_u8() != 0;
        let destination = SocksAddr::decode(buf).await?;
        Ok(UotRequest { is_connect, destination })
    }
}

impl SocksAddr {
    // Logic to encode a SOCKS address
    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), UotError> {
        match self {
            SocksAddr::Ip(addr) => {
                match addr.ip() {
                    IpAddr::V4(ip) => {
                        buf.put_u8(0x00); // AddressFamilyIPv4 from AddrParser
                        buf.put_slice(&ip.octets());
                    }
                    IpAddr::V6(ip) => {
                        buf.put_u8(0x01); // AddressFamilyIPv6 from AddrParser
                        buf.put_slice(&ip.octets());
                    }
                }
                buf.put_u16(addr.port());
            }
            SocksAddr::Domain(domain, port) => {
                buf.put_u8(0x02); // AddressFamilyFqdn from AddrParser
                if domain.len() > 255 {
                    return Err(UotError::Protocol("Domain name too long".into()));
                }
                buf.put_u8(domain.len() as u8);
                buf.put_slice(domain.as_bytes());
                buf.put_u16(*port);
            }
        }
        Ok(())
    }

    // Logic to decode a SOCKS address
    pub async fn decode(buf: &mut impl Buf) -> Result<Self, UotError> {
        // ... implementation to read SOCKS address ...
        // This is a bit complex, handling address types and lengths.
        // You'll read the type byte (0x00, 0x01, 0x02) and parse accordingly.
        // For example:
        let addr_type = buf.get_u8();
        match addr_type {
            0x00 => { // IPv4
                let ip = Ipv4Addr::new(buf.get_u8(), buf.get_u8(), buf.get_u8(), buf.get_u8());
                let port = buf.get_u16();
                Ok(SocksAddr::Ip(SocketAddr::new(IpAddr::V4(ip), port)))
            }
            0x01 => { // IPv6
                let ip = Ipv6Addr::new(
                    buf.get_u16(), buf.get_u16(), buf.get_u16(), buf.get_u16(),
                    buf.get_u16(), buf.get_u16(), buf.get_u16(), buf.get_u16()
                );
                let port = buf.get_u16();
                Ok(SocksAddr::Ip(SocketAddr::new(IpAddr::V6(ip), port)))
            }
            0x02 => { // Domain
                let len = buf.get_u8() as usize;
                let domain = String::from_utf8_lossy(&buf.chunk()[..len]).to_string();
                buf.advance(len);
                let port = buf.get_u16();
                Ok(SocksAddr::Domain(domain, port))
            }
            _ => Err(UotError::Protocol("Invalid address type".into())),
        }
    }
}
