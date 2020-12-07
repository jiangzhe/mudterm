use crate::error::{Error, Result};
use crate::proto::cli::Packet;
use crypto::digest::Digest;
use crypto::sha1::Sha1;
use rand::RngCore;
use std::net::TcpStream;
use std::time::Duration;

pub fn server_auth(mut conn: TcpStream, pass: &str) -> Result<TcpStream> {
    let orig_read_timeout = conn.read_timeout()?;
    let orig_write_timeout = conn.write_timeout()?;
    // reduce socket timeout to 5 seconds
    conn.set_read_timeout(Some(Duration::from_secs(5)))?;
    conn.set_write_timeout(Some(Duration::from_secs(5)))?;
    // send auth request with random seed
    let (seed, secret) = gen_secret(pass.as_bytes())?;
    let auth_req = Packet::AuthReq(seed);
    auth_req.write_to(&mut conn)?;
    // receive response and check
    let auth_resp = Packet::read_from(&mut conn)?;
    let mut auth_success = false;
    if let Packet::AuthResp(resp) = auth_resp {
        auth_success = resp == secret;
    }
    if !auth_success {
        let _ = Packet::Err(String::from("authentication failed")).write_to(&mut conn);
        return Err(Error::AuthError);
    } else {
        Packet::Ok.write_to(&mut conn)?;
    }
    // reset socket timeout
    conn.set_read_timeout(orig_read_timeout)?;
    conn.set_write_timeout(orig_write_timeout)?;
    log::debug!("server auth succeeds");
    Ok(conn)
}

pub fn client_auth(mut conn: TcpStream, pass: &str) -> Result<TcpStream> {
    let orig_read_timeout = conn.read_timeout()?;
    let orig_write_timeout = conn.write_timeout()?;
    // reduce socket timeout to 5 seconds
    conn.set_read_timeout(Some(Duration::from_secs(5)))?;
    conn.set_write_timeout(Some(Duration::from_secs(5)))?;
    // receive auth request
    let auth_req = Packet::read_from(&mut conn)?;
    if let Packet::AuthReq(req) = auth_req {
        // send resp to server
        let resp = calc_secret(pass.as_bytes(), &req)?;
        Packet::AuthResp(resp).write_to(&mut conn)?;
        // receive ok/err
        let msg = Packet::read_from(&mut conn)?;
        if msg != Packet::Ok {
            return Err(Error::AuthError);
        }
    }
    // reset socket timeout
    conn.set_read_timeout(orig_read_timeout)?;
    conn.set_write_timeout(orig_write_timeout)?;
    log::debug!("client auth succeeds");
    Ok(conn)
}

pub fn gen_secret(pass: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut seed = vec![0u8; 20];
    rand::thread_rng().fill_bytes(&mut seed);
    let secret = calc_secret(pass, &seed[..])?;
    Ok((seed, secret))
}

pub fn calc_secret(password: &[u8], seed: &[u8]) -> Result<Vec<u8>> {
    let mut hasher = Sha1::new();
    let stage1 = {
        let mut out = vec![0u8; 20];
        hasher.input(password);
        hasher.result(&mut out);
        out
    };
    hasher.reset();
    let stage2 = {
        let mut out = vec![0u8; 20];
        hasher.input(&stage1);
        hasher.result(&mut out);
        out
    };
    hasher.reset();

    let seed_hash = {
        let mut out = vec![0u8; 20];
        hasher.input(seed);
        hasher.input(&stage2);
        hasher.result(&mut out);
        out
    };
    let rst = seed_hash
        .iter()
        .zip(stage1.iter())
        .map(|(b1, b2)| b1 ^ b2)
        .collect();
    Ok(rst)
}
