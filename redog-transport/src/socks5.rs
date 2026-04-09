//! SOCKS5 protocol codec (RFC 1928)

use bytes::{BufMut, BytesMut};
use redog_core::Error;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const SOCKS5_VERSION: u8 = 0x05;
pub const NO_AUTH: u8 = 0x00;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Connect = 0x01,
    Bind = 0x02,
    UdpAssociate = 0x03,
}

impl TryFrom<u8> for Command {
    type Error = Error;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0x01 => Ok(Command::Connect),
            0x02 => Ok(Command::Bind),
            0x03 => Ok(Command::UdpAssociate),
            _ => Err(Error::Protocol(format!("unknown SOCKS5 command: {}", v))),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Address {
    Ip(SocketAddr),
    Domain(String, u16),
}

impl Address {
    pub fn host(&self) -> String {
        match self {
            Address::Ip(addr) => addr.ip().to_string(),
            Address::Domain(domain, _) => domain.clone(),
        }
    }

    pub fn port(&self) -> u16 {
        match self {
            Address::Ip(addr) => addr.port(),
            Address::Domain(_, port) => *port,
        }
    }

    pub fn ip(&self) -> Option<IpAddr> {
        match self {
            Address::Ip(addr) => Some(addr.ip()),
            Address::Domain(_, _) => None,
        }
    }
}

/// Perform SOCKS5 server-side handshake: read method negotiation + connect request
pub async fn server_handshake<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
) -> Result<(Command, Address), Error> {
    // 1. Read method negotiation
    let ver = read_u8(stream).await?;
    if ver != SOCKS5_VERSION {
        return Err(Error::Protocol(format!(
            "unsupported SOCKS version: {}",
            ver
        )));
    }

    let nmethods = read_u8(stream).await? as usize;
    let mut methods = vec![0u8; nmethods];
    stream.read_exact(&mut methods).await?;

    // 2. Reply: no auth
    stream.write_all(&[SOCKS5_VERSION, NO_AUTH]).await?;

    // 3. Read connect request
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;
    if header[0] != SOCKS5_VERSION {
        return Err(Error::Protocol("invalid SOCKS5 request version".into()));
    }

    let cmd = Command::try_from(header[1])?;
    let atyp = header[3];

    // 4. Parse address
    let addr = read_address(stream, atyp).await?;

    // 5. Send success reply
    let reply = [
        SOCKS5_VERSION,
        0x00, // success
        0x00, // reserved
        0x01, // IPv4
        0, 0, 0, 0, // bind addr
        0, 0, // bind port
    ];
    stream.write_all(&reply).await?;

    Ok((cmd, addr))
}

/// Encode a SOCKS5 address into bytes
pub fn encode_address(addr: &Address) -> BytesMut {
    let mut buf = BytesMut::new();
    match addr {
        Address::Ip(SocketAddr::V4(v4)) => {
            buf.put_u8(0x01);
            buf.extend_from_slice(&v4.ip().octets());
            buf.put_u16(v4.port());
        }
        Address::Ip(SocketAddr::V6(v6)) => {
            buf.put_u8(0x04);
            buf.extend_from_slice(&v6.ip().octets());
            buf.put_u16(v6.port());
        }
        Address::Domain(domain, port) => {
            buf.put_u8(0x03);
            buf.put_u8(domain.len() as u8);
            buf.extend_from_slice(domain.as_bytes());
            buf.put_u16(*port);
        }
    }
    buf
}

async fn read_address<S: AsyncRead + Unpin>(
    stream: &mut S,
    atyp: u8,
) -> Result<Address, Error> {
    match atyp {
        0x01 => {
            // IPv4
            let mut octets = [0u8; 4];
            stream.read_exact(&mut octets).await?;
            let port = read_u16(stream).await?;
            Ok(Address::Ip(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::from(octets)),
                port,
            )))
        }
        0x03 => {
            // Domain
            let len = read_u8(stream).await? as usize;
            let mut domain_buf = vec![0u8; len];
            stream.read_exact(&mut domain_buf).await?;
            let domain = String::from_utf8(domain_buf)
                .map_err(|e| Error::Protocol(format!("invalid domain: {}", e)))?;
            let port = read_u16(stream).await?;
            Ok(Address::Domain(domain, port))
        }
        0x04 => {
            // IPv6
            let mut octets = [0u8; 16];
            stream.read_exact(&mut octets).await?;
            let port = read_u16(stream).await?;
            Ok(Address::Ip(SocketAddr::new(
                IpAddr::V6(Ipv6Addr::from(octets)),
                port,
            )))
        }
        _ => Err(Error::Protocol(format!(
            "unsupported SOCKS5 address type: {}",
            atyp
        ))),
    }
}

async fn read_u8<S: AsyncRead + Unpin>(stream: &mut S) -> Result<u8, Error> {
    let mut buf = [0u8; 1];
    stream.read_exact(&mut buf).await?;
    Ok(buf[0])
}

async fn read_u16<S: AsyncRead + Unpin>(stream: &mut S) -> Result<u16, Error> {
    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).await?;
    Ok(u16::from_be_bytes(buf))
}
