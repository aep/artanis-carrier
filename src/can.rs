extern crate socketcan;

use super::isotp;
use mio_extras::channel;



pub fn rq(addr: u32, data: Vec<u8>) -> channel::Receiver<Vec<u8>> {
    let (tx, rx) = channel::channel();

    std::thread::spawn(move || {
        if let Err(e) = rq_thread(addr, data, tx) {
            log::error!("{}", e);
        }
    });

    rx
}


fn map_err<E>(e: E) -> std::io::Error
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    std::io::Error::new(std::io::ErrorKind::Other, e)
}

pub fn rq_thread(addr: u32, data: Vec<u8>, tx: channel::Sender<Vec<u8>> )  -> Result<(), std::io::Error> {
    let sock = socketcan::CANSocket::open("can0").map_err(map_err)?;
    sock.set_write_timeout(std::time::Duration::from_secs(1))?;
    sock.set_read_timeout(std::time::Duration::from_secs(1))?;

    println!(">> {:x}, {:02x?}", addr, data);
    let d = isotp::send(data);

    for d in d {
        sock.write_frame(&socketcan::CANFrame::new(addr, &d, false, false).map_err(map_err)?).map_err(map_err)?;
    }


    let mut len = 0;
    let mut data = Vec::new();
    println!("waiting to recv can");
    loop {
        let frame = sock.read_frame()?;
        println!("<< {:02x?}", frame);

        let d1 = frame.data();
        if d1.len() != 8 {
            continue;
        }



        if len == 0 {
            if d1[0] == 0x10 {
                len = d1[1] as usize;
                data.extend_from_slice(&d1[2..]);

                //flow control continue
                let d = [0x30,00,00,00,00,00,00,00];
                sock.write_frame(&socketcan::CANFrame::new(addr, &d, false, false).map_err(map_err)?).map_err(map_err)?;

            } else {
                len = d1[0] as usize;
                tx.send(d1[1..len].to_vec()).map_err(map_err)?;
                return Ok(());
            }
        } else {
            data.extend_from_slice(&d1[1..]);
            if data.len() >= len {
                tx.send(data[..len].to_vec()).map_err(map_err)?;
                return Ok(());
            }
        }
    }
}


