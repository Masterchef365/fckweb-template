#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! rcgen = "0.13.2"
//! anyhow = "1"
//! sha256 = "1.5.0"
//! ```

use rcgen::{generate_simple_self_signed, CertifiedKey};
fn main() -> anyhow::Result<()> {
    // Generate a certificate that's valid for "localhost" and "hello.world.example"
    let subject_alt_names = vec!["127.0.0.1".to_string(),
    	"localhost".to_string()];

    let CertifiedKey { cert, key_pair } = generate_simple_self_signed(subject_alt_names)?;
    //println!("{}", cert.pem());
    //println!("{}", key_pair.serialize_pem());
    let hex = sha256::digest(cert.der().as_ref());

    std::fs::write("common/src/localhost.crt", cert.pem())?;
    std::fs::write("common/src/localhost.hex", hex)?;
    std::fs::write("server/src/localhost.key", key_pair.serialize_pem())?;

    Ok(())
}