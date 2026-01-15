use std::{fs::File, io::BufReader};

use rustls::server::ServerConfig;
use rustls_pemfile::{certs, private_key};
use tracing::error;

pub fn load_certs(cert: &str, key: &str) -> ServerConfig {
    // init server config builder with safe defaults
    let config = ServerConfig::builder().with_no_client_auth();

    // load TLS key/cert files
    let cert_file = &mut BufReader::new(File::open(cert).expect("Could not open cert file"));
    let key_file = &mut BufReader::new(File::open(key).expect("Could not open key file"));

    // convert files to key/cert objects
    let cert_chain = certs(cert_file)
        .map(|v| v.expect("Could not parse cert"))
        .collect::<Vec<_>>();

    let keys = private_key(key_file).expect("Could not parse key");

    // exit if no keys could be parsed
    if let Some(key) = keys {
        config
            .with_single_cert(cert_chain, key)
            .expect("Could not load key/cert")
    } else {
        error!("Could not locate PKCS 8 private keys.");
        std::process::exit(1);
    }
}
