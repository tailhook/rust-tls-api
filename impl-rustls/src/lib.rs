use std::fmt;
use std::result;
use std::str;
use std::sync::Arc;

use crate::handshake::HandshakeFuture;
use rustls::NoClientAuth;
use std::future::Future;
use std::pin::Pin;
use tls_api::runtime::AsyncRead;
use tls_api::runtime::AsyncWrite;
use tls_api::Error;
use tls_api::PrivateKey;
use tls_api::Result;
use tls_api::X509Cert;
use webpki::DNSNameRef;

mod handshake;
mod stream;

pub(crate) use stream::TlsStream;
use tls_api::async_as_sync::AsyncIoAsSyncIo;

pub struct TlsConnectorBuilder {
    pub config: rustls::ClientConfig,
    pub verify_hostname: bool,
}
pub struct TlsConnector {
    pub config: Arc<rustls::ClientConfig>,
}

pub struct TlsAcceptorBuilder(pub rustls::ServerConfig);
pub struct TlsAcceptor(pub Arc<rustls::ServerConfig>);

impl tls_api::TlsConnectorBuilder for TlsConnectorBuilder {
    type Connector = TlsConnector;

    type Underlying = rustls::ClientConfig;

    fn underlying_mut(&mut self) -> &mut rustls::ClientConfig {
        &mut self.config
    }

    const SUPPORTS_ALPN: bool = true;

    fn set_alpn_protocols(&mut self, protocols: &[&[u8]]) -> Result<()> {
        self.config.alpn_protocols = protocols.into_iter().map(|p: &&[u8]| p.to_vec()).collect();
        Ok(())
    }

    fn set_verify_hostname(&mut self, verify: bool) -> Result<()> {
        if !verify {
            struct NoCertificateVerifier;

            impl rustls::ServerCertVerifier for NoCertificateVerifier {
                fn verify_server_cert(
                    &self,
                    _roots: &rustls::RootCertStore,
                    _presented_certs: &[rustls::Certificate],
                    _dns_name: webpki::DNSNameRef,
                    _ocsp_response: &[u8],
                ) -> result::Result<rustls::ServerCertVerified, rustls::TLSError> {
                    Ok(rustls::ServerCertVerified::assertion())
                }
            }

            self.config
                .dangerous()
                .set_certificate_verifier(Arc::new(NoCertificateVerifier));
            self.verify_hostname = false;
        } else {
            if !self.verify_hostname {
                return Err(Error::new_other(
                    "cannot set_verify_hostname(true) after set_verify_hostname(false)",
                ));
            }
        }

        Ok(())
    }

    fn add_root_certificate(&mut self, cert: &tls_api::X509Cert) -> Result<&mut Self> {
        let cert = rustls::Certificate(cert.as_bytes().to_vec());
        self.config
            .root_store
            .add(&cert)
            .map_err(|e| Error::new_other(&format!("{:?}", e)))?;
        Ok(self)
    }

    fn build(mut self) -> Result<TlsConnector> {
        if self.config.root_store.is_empty() {
            self.config
                .root_store
                .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
        }
        Ok(TlsConnector {
            config: Arc::new(self.config),
        })
    }
}

impl tls_api::TlsConnector for TlsConnector {
    type Builder = TlsConnectorBuilder;

    fn builder() -> Result<TlsConnectorBuilder> {
        Ok(TlsConnectorBuilder {
            config: rustls::ClientConfig::new(),
            verify_hostname: true,
        })
    }

    fn connect<'a, S>(
        &'a self,
        domain: &'a str,
        stream: S,
    ) -> Pin<Box<dyn Future<Output = tls_api::Result<tls_api::TlsStream<S>>> + Send + 'a>>
    where
        S: AsyncRead + AsyncWrite + fmt::Debug + Unpin + Send + Sync + 'static,
    {
        let dns_name =
            match DNSNameRef::try_from_ascii_str(domain).map_err(|e| tls_api::Error::new(e)) {
                Ok(dns_name) => dns_name,
                Err(e) => return Box::pin(async { Err(e) }),
            };
        let tls_stream = TlsStream {
            stream: AsyncIoAsSyncIo::new(stream),
            session: rustls::ClientSession::new(&self.config, dns_name),
        };

        Box::pin(HandshakeFuture::MidHandshake(tls_stream))
    }
}

// TlsAcceptor and TlsAcceptorBuilder

impl TlsAcceptorBuilder {
    pub fn from_cert_and_key(cert: &X509Cert, key: &PrivateKey) -> Result<TlsAcceptorBuilder> {
        let mut config = rustls::ServerConfig::new(Arc::new(NoClientAuth));
        let cert = rustls::Certificate(cert.as_bytes().to_vec());
        config
            .set_single_cert(vec![cert], rustls::PrivateKey(key.as_bytes().to_vec()))
            .map_err(tls_api::Error::new)?;
        Ok(TlsAcceptorBuilder(config))
    }
}

impl tls_api::TlsAcceptorBuilder for TlsAcceptorBuilder {
    type Acceptor = TlsAcceptor;

    type Underlying = rustls::ServerConfig;

    // TODO: https://github.com/sfackler/rust-openssl/pull/646
    const SUPPORTS_ALPN: bool = true;

    fn set_alpn_protocols(&mut self, protocols: &[&[u8]]) -> Result<()> {
        self.0.alpn_protocols = protocols.into_iter().map(|p| p.to_vec()).collect();
        Ok(())
    }

    fn underlying_mut(&mut self) -> &mut rustls::ServerConfig {
        &mut self.0
    }

    fn build(self) -> Result<TlsAcceptor> {
        Ok(TlsAcceptor(Arc::new(self.0)))
    }
}

impl tls_api::TlsAcceptor for TlsAcceptor {
    type Builder = TlsAcceptorBuilder;

    fn accept<'a, S>(
        &'a self,
        stream: S,
    ) -> Pin<Box<dyn Future<Output = tls_api::Result<tls_api::TlsStream<S>>> + Send + 'a>>
    where
        S: AsyncRead + AsyncWrite + fmt::Debug + Unpin + Send + Sync + 'static,
    {
        let tls_stream = crate::TlsStream {
            stream: AsyncIoAsSyncIo::new(stream),
            session: rustls::ServerSession::new(&self.0),
        };

        Box::pin(HandshakeFuture::MidHandshake(tls_stream))
    }
}
