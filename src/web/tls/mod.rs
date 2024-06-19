use std::fs::File;
use std::io::BufReader;

use rustls::server::ServerConfig;
use rustls_pemfile::{certs, private_key};

pub fn load_certs(cert: &str, key: &str) -> ServerConfig {

    // init server config builder with safe defaults
    let config = ServerConfig::builder()
        .with_no_client_auth();

    // load TLS key/cert files
    let cert_file = &mut BufReader::new(File::open(cert).unwrap());
    let key_file = &mut BufReader::new(File::open(key).unwrap());

    // convert files to key/cert objects
    let cert_chain = certs(cert_file)
        .map(|v| v.unwrap())
        .collect::<Vec<_>>();

    let keys = private_key(key_file).unwrap();

    // exit if no keys could be parsed
    if let Some(key) = keys {
        config.with_single_cert(cert_chain, key).unwrap()
    } else {
        eprintln!("Could not locate PKCS 8 private keys.");
        std::process::exit(1);
    }
}
