use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::error::Error;

/// TCP proxy stream trait
/// Combines AsyncRead + AsyncWrite with proxy chain tracking
pub trait ProxyStream: AsyncRead + AsyncWrite + Unpin + Send + Sync {
    /// Get the proxy chain for debugging
    fn chains(&self) -> &[String];

    /// Append a node to the proxy chain
    fn append_chain(&mut self, name: String);
}

/// UDP proxy datagram trait
#[async_trait]
pub trait ProxyDatagram: Send + Sync {
    /// Send a UDP packet to the target address
    async fn send_to(&self, buf: &[u8], target: &SocketAddr) -> Result<usize, Error>;

    /// Receive a UDP packet
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), Error>;

    /// Close the datagram session
    async fn close(&self) -> Result<(), Error>;
}

// ── Concrete implementations ──

/// A tracked stream wrapping any AsyncRead + AsyncWrite
pub struct TrackedStream<S> {
    inner: S,
    chains: Vec<String>,
}

impl<S> TrackedStream<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            chains: Vec::new(),
        }
    }

    pub fn with_chain(inner: S, chain: String) -> Self {
        Self {
            inner,
            chains: vec![chain],
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send + Sync> ProxyStream for TrackedStream<S> {
    fn chains(&self) -> &[String] {
        &self.chains
    }

    fn append_chain(&mut self, name: String) {
        self.chains.push(name);
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for TrackedStream<S> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for TrackedStream<S> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// A tracked UDP socket
pub struct TrackedDatagram {
    inner: tokio::net::UdpSocket,
}

impl TrackedDatagram {
    pub fn new(inner: tokio::net::UdpSocket) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl ProxyDatagram for TrackedDatagram {
    async fn send_to(&self, buf: &[u8], target: &SocketAddr) -> Result<usize, Error> {
        Ok(self.inner.send_to(buf, target).await?)
    }

    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), Error> {
        Ok(self.inner.recv_from(buf).await?)
    }

    async fn close(&self) -> Result<(), Error> {
        Ok(())
    }
}
