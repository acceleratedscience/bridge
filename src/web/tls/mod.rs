use rustls::server::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};

pub fn load_certs(cert: &str, key: &str) -> ServerConfig {
    // convert files to key/cert objects
    let cert_chain = CertificateDer::pem_file_iter(cert)
        .expect("Could not read cert file")
        .map(|v| v.expect("Could not parse cert"))
        .collect::<Vec<_>>();

    let keys = PrivateKeyDer::from_pem_file(key).expect("Could not parse key");

    // exit if no keys could be parsed
    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, keys)
        .expect("Could not create TLS config")
}
