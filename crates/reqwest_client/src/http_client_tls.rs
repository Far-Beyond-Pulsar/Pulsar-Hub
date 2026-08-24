//! Self-contained TLS configuration for all HTTP clients.
//!
//! Everything is bundled so that TLS behaves identically on every machine:
//!
//! - **Roots**: Mozilla's root program compiled into the binary
//!   (`webpki-roots`) instead of the machine's certificate store, which AV
//!   tools, group policy and stale Windows installs can mutate.
//! - **Crypto provider**: `ring` instead of `aws_lc_rs`, which fails on older
//!   CPUs lacking BMI2/ADX instructions.
//!
//! The only remaining environmental dependency is a roughly correct system
//! clock. `PULSAR_INSECURE_TLS=1` disables verification entirely as a
//! diagnostic escape hatch.

use std::sync::{Arc, OnceLock};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error, RootCertStore, SignatureScheme};

static TLS_CONFIG: OnceLock<ClientConfig> = OnceLock::new();

pub fn tls_config() -> ClientConfig {
    TLS_CONFIG.get_or_init(build_config).clone()
}

fn insecure_tls_enabled() -> bool {
    std::env::var("PULSAR_INSECURE_TLS").as_deref() == Ok("1")
}

fn build_config() -> ClientConfig {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
        .expect("supported protocol versions");

    if insecure_tls_enabled() {
        log::warn!("PULSAR_INSECURE_TLS=1 — certificate verification disabled");
        return builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAnyServerCert { provider }))
            .with_no_client_auth();
    }

    let roots = RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let verifier = WebPkiServerVerifier::builder(Arc::new(roots))
        .build()
        .expect("webpki verifier with bundled roots");

    builder
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth()
}

/// Verifier used when `PULSAR_INSECURE_TLS=1`: accepts any certificate chain
/// while still negotiating real handshake signatures through `ring`.
#[derive(Debug)]
struct AcceptAnyServerCert {
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for AcceptAnyServerCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}
