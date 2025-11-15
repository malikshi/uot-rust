// src/server.rs
use crate::protocol::{SocksAddr, UotRequest};
use crate::UotError;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};

// Translates `ServerConn`'s logic
pub async fn handle_connection(
    mut tcp_stream: TcpStream,
    udp_socket: Arc<UdpSocket>,
    version: u8,
) -> Result<(), UotError> {
    
    // 1. Read the initial UoT Request
    let request = UotRequest::decode(&mut tcp_stream).await?;
    let UotRequest { is_connect, destination } = request;

    let (mut tcp_reader, mut tcp_writer) = tcp_stream.split();
    let udp_socket_read = udp_socket.clone();
    let udp_socket_write = udp_socket;

    // 2. Spawn TCP-to-UDP loop (Go's loopInput)
    let tcp_to_udp = tokio::spawn(async move {
        let mut buf = vec![0u8; 65535]; // Max UDP packet size
        loop {
            // 1. Read destination (if not connect-mode)
            let target_addr = if is_connect {
                destination.clone()
            } else {
                SocksAddr::decode(&mut tcp_reader).await?
            };

            // 2. Read length
            let len = tcp_reader.read_u16().await? as usize;

            // 3. Read payload
            tcp_reader.read_exact(&mut buf[..len]).await?;

            // 4. Resolve address and send UDP packet
            let udp_target = match target_addr {
                SocksAddr::Ip(addr) => addr,
                SocksAddr::Domain(domain, port) => {
                    tokio::net::lookup_host((domain, port)).await?
                        .next()
                        .ok_or(UotError::ResolutionFailed)?
                }
            };
            udp_socket_write.send_to(&buf[..len], udp_target).await?;
        }
        // Help type inference
        #[allow(unreachable_code)]
        Ok::<(), UotError>(())
    });

    // 3. Spawn UDP-to-TCP loop (Go's loopOutput)
    let udp_to_tcp = tokio::spawn(async move {
        let mut buf = vec![0u8; 65535];
        loop {
            // 1. Read from UDP
            let (len, from_addr) = udp_socket_read.recv_from(&mut buf).await?;

            // 2. Frame and write to TCP
            let mut frame = BytesMut::new();
            if !is_connect {
                SocksAddr::Ip(from_addr).encode(&mut frame)?;
            }
            frame.put_u16(len as u16);
            frame.put_slice(&buf[..len]);

            tcp_writer.write_all(&frame).await?;
        }
        #[allow(unreachable_code)]
        Ok::<(), UotError>(())
    });

    // Wait for either loop to exit
    tokio::select! {
        res = tcp_to_udp => res??,
        res = udp_to_tcp => res??,
    };

    Ok(())
}
