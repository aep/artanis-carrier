#![feature(generators, generator_trait)]

use carrier;
use env_logger;
use carrier::osaka::{self, osaka};

mod can;
mod isotp;

fn main() -> Result<(), carrier::Error> {
    if let Err(_) = std::env::var("RUST_LOG") {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let mut args = std::env::args();
    args.next();
    match args.next().as_ref().map(|v|v.as_str()) {
        Some("publish") => {
            let config = carrier::config::load().unwrap();

            let poll            = carrier::osaka::Poll::new();
            let config          = carrier::config::load()?;
            let mut publisher   = carrier::publisher::new(config)
                .route("/v0/shell",                             None,       carrier::publisher::shell::main)
                .route("/v0/sft",                               None,       carrier::publisher::sft::main)
                .route("/v0/reboot",                            None,       reboot)
                .route("/v0/ota",                               None,       carrier::publisher::openwrt::ota)
                .route("/v2/carrier.certificate.v1/authorize",  Some(1024), carrier::publisher::authorization::main)
                .route("/v2/carrier.sysinfo.v1/sysinfo",        None,       carrier::publisher::sysinfo::sysinfo)
                .route("/v2/carrier.sysinfo.v1/netsurvey",      None,       carrier::publisher::openwrt::netsurvey)
                .route("/v1/devguard.artanis.v1/can/obd",       None, move |p,h,i,s| can_obd(p,h,i,s))
                .route("/v2/carrier.sysinfo.v1/sysinfo",        None, carrier::publisher::sysinfo::sysinfo)
                .with_disco("artanis".to_string(), "SIM".to_string())
                .publish(poll);
            publisher.run()?;
        }
        Some("identity") => {
            let config = carrier::config::load()?;
            println!("{}", config.secret.identity());
        }
        Some("lolcast") => {
            let config = carrier::config::load()?;
            let msg = format!("CR1:BTN:{}", config.secret.identity()).as_bytes().to_vec();
            let socket = std::net::UdpSocket::bind("224.0.0.251:0")?;
            socket.set_broadcast(true).expect("set_broadcast call failed");
            socket.send_to(&msg, "224.0.0.251:8444").expect("couldn't send message");
            socket.send_to(&msg, "0.0.0.0:8444").expect("couldn't send message");
        }
        _ => {
            eprintln!("cmds: publish, identity, lolcast");
        }
    }

    Ok(())

}


pub fn decode_hex(s: &str) -> Result<Vec<u8>, std::num::ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}


pub fn can_obd(
    poll: carrier::osaka::Poll,
    headers: carrier::headers::Headers,
    _identity: &carrier::identity::Identity,
    mut stream: carrier::endpoint::Stream,
) -> Option<carrier::osaka::Task<()>> {

    let addr = if let Some(v) = headers.get(b"addr") {
        match u32::from_str_radix(&String::from_utf8_lossy(&v), 16) {
            Ok(v) => v,
            Err(e) => {
                stream.send(carrier::headers::Headers::with_error(400, format!("{}", e).as_bytes().to_vec()).encode());
                return None;
            }
        }
    } else {
        stream.send(carrier::headers::Headers::with_error(400, "addr required").encode());
        return None;
    };


    let q = if let Some(v) = headers.get(b"x") {
        match decode_hex(&String::from_utf8_lossy(&v)) {
            Ok(v) => {
                v
            },
            Err(e) => {
                stream.send(carrier::headers::Headers::with_error(400, format!("{}", e).as_bytes().to_vec()).encode());
                return None;
            }
        }
    } else {
        stream.send(carrier::headers::Headers::with_error(400, "x required").encode());
        return None;
    };

    if let Some(v) = headers.get(b"bus") {
        if v == b"0" {
            return Some(can_obd_s(poll, stream, addr, q.to_vec()));
        } else if v == b"1" {
            stream.send(carrier::headers::Headers::with_error(408, "no response").encode());
            return None;
        } else {
            stream.send(carrier::headers::Headers::with_error(400, "invalid bus").encode());
            return None;
        }
    } else {
        stream.send(carrier::headers::Headers::with_error(400, "bus required").encode());
        return None;
    }
}



#[osaka]
fn can_obd_s(
    poll: carrier::osaka::Poll,
    mut stream: carrier::endpoint::Stream,
    addr: u32,
    q: Vec<u8>,
) {
    let now =  std::time::Instant::now();
    let rx = can::rq(addr, q);
    let token = poll.register(&rx, carrier::mio::Ready::readable(), carrier::mio::PollOpt::empty()).unwrap();

    loop {
        std::thread::sleep(std::time::Duration::from_millis(10));
        if now.elapsed().as_secs() > 5 {
            stream.send(carrier::headers::Headers::with_error(408, "deadlock").encode());
            return;
        }
        match rx.try_recv() {
            Ok(b) => {
                stream.send(carrier::headers::Headers::ok().encode());
                stream.send(b);
                return;
            },
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                yield poll.again(token.clone(), Some(std::time::Duration::from_secs(1)));
            },
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                stream.send(carrier::headers::Headers::with_error(408, "no response").encode());
                return;
            }
        }
    }
}


pub fn reboot(
    _poll: osaka::Poll,
    _headers: carrier::headers::Headers,
    _identity: &carrier::identity::Identity,
    mut stream: carrier::endpoint::Stream,
) -> Option<osaka::Task<()>> {
    use std::process::Command;
    stream.send(carrier::headers::Headers::ok().encode());
    Command::new("/bin/sh")
        .args(vec!["-c" , "reboot"])
        .spawn().unwrap();
    None
}


