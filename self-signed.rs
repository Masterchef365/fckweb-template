#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! rcgen = "0.13.2"
//! anyhow = "1"
//! sha256 = "1.5.0"
//! time = "0.3"
//! ```

fn main() -> anyhow::Result<()> {
    // Generate a certificate that's valid for "localhost" and "hello.world.example"
    let subject_alt_names = vec!["127.0.0.1".to_string(),
    	"localhost".to_string()];

    let key_pair = rcgen::KeyPair::generate()?;
    let mut cfg = rcgen::CertificateParams::new(subject_alt_names)?;
    let now = time::OffsetDateTime::now_utc();
    cfg.not_before = now - time::Duration::days(2);
    cfg.not_after = now + time::Duration::days(10);
    dbg!(cfg.not_before);
    dbg!(cfg.not_after);
	let cert = cfg.self_signed(&key_pair)?;

    //println!("{}", cert.pem());
    //println!("{}", key_pair.serialize_pem());
    let hex = sha256::digest(cert.der().as_ref());

    std::fs::write("common/src/localhost.crt", cert.pem())?;
    std::fs::write("common/src/localhost.hex", hex)?;
    std::fs::write("server/src/localhost.key", key_pair.serialize_pem())?;

    Ok(())
}
