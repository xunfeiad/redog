use std::net::SocketAddr;
use tokio::net::TcpStream;

/// Connect to address with timeout
pub async fn tcp_connect(addr: SocketAddr) -> std::io::Result<TcpStream> {
    let stream = TcpStream::connect(addr).await?;
    stream.set_nodelay(true)?;
    Ok(stream)
}

/// Connect to address with optional binding
pub async fn tcp_connect_with_bind(
    addr: SocketAddr,
    bind_addr: Option<SocketAddr>,
) -> std::io::Result<TcpStream> {
    if let Some(bind) = bind_addr {
        let socket = if addr.is_ipv4() {
            tokio::net::TcpSocket::new_v4()?
        } else {
            tokio::net::TcpSocket::new_v6()?
        };
        socket.bind(bind)?;
        let stream = socket.connect(addr).await?;
        stream.set_nodelay(true)?;
        Ok(stream)
    } else {
        tcp_connect(addr).await
    }
}
