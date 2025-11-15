// src/client.rs
use crate::protocol::{SocksAddr, UotRequest, VERSION};
use crate::UotError;
use bytes::BytesMut;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpStream, ToSocketAddrs};

// UotConn translates Go's `Conn` type
pub struct UotConn {
    stream: TcpStream,
    is_connect: bool,
    destination: SocksAddr,
    request_written: bool,
    is_lazy: bool,
}

impl UotConn {
    // This translates client.DialContext
    pub async fn connect(
        addr: impl ToSocketAddrs,
        version: u8,
        is_connect: bool,
        destination: SocksAddr,
    ) -> Result<Self, UotError> {
        Self::connect_internal(addr, version, is_connect, destination, false).await
    }

    // This translates client.DialEarlyConn
    // by creating a "lazy" connection
    pub async fn connect_lazy(
        addr: impl ToSocketAddrs,
        version: u8,
        is_connect: bool,
        destination: SocksAddr,
    ) -> Result<Self, UotError> {
        Self::connect_internal(addr, version, is_connect, destination, true).await
    }

    // Internal connect logic
    async fn connect_internal(
        addr: impl ToSocketAddrs,
        version: u8,
        is_connect: bool,
        destination: SocksAddr,
        is_lazy: bool,
    ) -> Result<Self, UotError> {
        if version != 0 && version != VERSION && version != crate::protocol::LEGACY_VERSION {
            return Err(UotError::UnknownVersion(version));
        }

        let mut stream = TcpStream::connect(addr).await?;
        let request_written = false;

        let mut conn = Self {
            stream,
            is_connect,
            destination,
            request_written,
            is_lazy,
        };

        if !is_lazy {
            conn.write_request().await?;
        }

        Ok(conn)
    }

    // Helper to write the initial request
    async fn write_request(&mut self) -> Result<(), UotError> {
        if self.request_written {
            return Ok(());
        }
        let request = UotRequest {
            is_connect: self.is_connect,
            destination: self.destination.clone(),
        };
        let mut buf = BytesMut::new();
        request.encode(&mut buf)?;
        self.stream.write_all(&buf).await?;
        self.request_written = true;
        Ok(())
    }

    // Translates conn.WriteTo
    pub async fn send_to(&mut self, payload: &[u8], target: &SocksAddr) -> Result<usize, UotError> {
        // Handle lazy write
        if !self.request_written && self.is_lazy {
            self.write_request().await?;
        }

        let mut buf = BytesMut::new();
        if !self.is_connect {
            // Write destination address
            target.encode(&mut buf)?;
        }
        // Write 2-byte length
        buf.put_u16(payload.len() as u16);
        // Write payload
        buf.put_slice(payload);

        self.stream.write_all(&buf).await?;
        Ok(payload.len())
    }

    // Translates conn.ReadFrom
    pub async fn recv_from(&mut self, buf: &mut [u8]) -> Result<(usize, SocksAddr), UotError> {
        let mut stream = &mut self.stream;
        
        // 1. Read destination address
        let destination = if self.is_connect {
            self.destination.clone()
        } else {
            // This is complex. You need to read bytes, parse SocksAddr,
            // which isn't easy with a simple buffer.
            // Using a `tokio_util::codec::Decoder` is the *right* way
            // to handle this stateful, framed reading.
            // For a simple example, we'll assume a helper.
            SocksAddr::decode(&mut stream).await?
        };

        // 2. Read length
        let len = stream.read_u16().await? as usize;

        // 3. Read payload
        if buf.len() < len {
            return Err(UotError::Protocol("Buffer too small".into()));
        }
        stream.read_exact(&mut buf[..len]).await?;

        Ok((len, destination))
    }
}