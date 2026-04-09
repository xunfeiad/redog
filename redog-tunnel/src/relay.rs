use redog_core::conn::ProxyStream;
use redog_core::error::Error;
use tokio::io::copy_bidirectional;

/// Bidirectional TCP relay between two streams
/// On Linux, tokio may use splice(2) for zero-copy when both sides are sockets
pub async fn relay_tcp(
    mut left: Box<dyn ProxyStream>,
    mut right: Box<dyn ProxyStream>,
) -> Result<(u64, u64), Error> {
    let result = copy_bidirectional(&mut left, &mut right).await?;
    Ok(result)
}
