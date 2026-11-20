//! Connection authority: which credential a peer presented, checked again on
//! every request. No persisted registry, operator rights or token diagnostics.
use crate::{Access, Credential, Error, Result, Stream};
use serde_json::json;
use std::sync::Arc;
use subtle::ConstantTimeEq;

/// The daemon's peer admission: per-launch credentials plus the revocable
/// operator write grants.
#[derive(Clone, Default)]
pub struct Peers {
    pub writes: crate::write_grant::Registry,
}
/// No secret is copied into the connection handle; validity is checked per request.
#[derive(Clone)]
pub struct Peer {
    access: Access,
    write: Option<Arc<crate::write_grant::Grant>>,
}
impl Peer {
    pub fn access(&self) -> Access {
        self.access.clone()
    }
    pub fn check(&self) -> Result<()> {
        self.write.as_ref().map_or(Ok(()), |g| g.check())
    }
}
impl Peers {
    pub async fn accept(&self, stream: &mut Stream, credentials: &[Credential]) -> Result<Peer> {
        let hello: crate::Hello = serde_json::from_value(crate::read_frame(stream).await?)
            .map_err(|_| Error::Unauthorized)?;
        if hello.api_version != crate::API_VERSION {
            return Err(Error::Version);
        }
        let peer = if let Some(c) = credentials
            .iter()
            .find(|c| bool::from(c.token.ct_eq(&hello.credential)))
        {
            Peer {
                access: c.access.clone(),
                write: None,
            }
        } else {
            let g = self
                .writes
                .authenticate(&hello.credential)
                .map_err(|_| Error::Unauthorized)?;
            Peer {
                access: g.access(),
                write: Some(g),
            }
        };
        peer.check()?;
        crate::write_frame(
            stream,
            &json!({"api_version":crate::API_VERSION,"ready":true,"access":peer.access}),
        )
        .await?;
        Ok(peer)
    }
}
