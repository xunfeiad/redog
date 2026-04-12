pub mod http;
pub mod http_parse;
pub mod mixed;
pub mod socks;

use redog_core::metadata::Metadata;
use tokio::net::TcpStream;

/// A connection accepted by a listener, ready to be handled by the tunnel
pub struct InboundConnection {
    pub stream: TcpStream,
    pub metadata: Metadata,
}
