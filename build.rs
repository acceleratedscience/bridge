use rcgen::{CertifiedKey, generate_simple_self_signed};
use std::{
    env,
    fs::{File, create_dir_all},
    io::Write,
    path::Path,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build.rs");

    let profile = env::var("PROFILE").unwrap_or_default();
    if profile != "debug" {
        return Ok(());
    }

    let cert_path = Path::new("certs/fullchain.cer");
    let key_path = Path::new("certs/private.key");

    if cert_path.exists() && key_path.exists() {
        return Ok(());
    }

    create_dir_all("certs")?;

    let subject_alt_names = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "127.225.255.254".to_string(),
    ];

    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(subject_alt_names).unwrap();

    File::create(cert_path)?.write_all(cert.pem().as_bytes())?;
    File::create(key_path)?.write_all(signing_key.serialize_pem().as_bytes())?;

    println!("cargo:warning=Fresh dev TLS certificates generated in ./certs");
    Ok(())
}

