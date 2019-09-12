pub fn send(mut b: Vec<u8>) -> Vec<Vec<u8>> {
    if b.len() < 8 {
        b.insert(0, b.len() as u8);
        if b.len() < 8 {
            b.extend(vec![0xff; 8 - b.len()]);
        }
        return vec![b];
    }

    // too lazy to implement bitfields
    assert!(b.len() < 255);
    let mut b1 = vec![
        0x10,
        b.len() as u8
    ];

    b1.extend_from_slice(&b[0..6]);
    let mut bb = vec![b1];


    let mut at = 6;
    while at < b.len() {
        let mut b2 = vec![0x2 | (bb.len() << 4) as u8];
        let to = std::cmp::min(b.len(), at+7);
        b2.extend_from_slice(&b[at..to]);
        if b2.len() < 8 {
            b2.extend(vec![0xff; 8 - b2.len()]);
        }
        at += 7;
        bb.push(b2);
    }
    bb
}

#[test]
pub fn hyundai() {
    let b = vec![
        0x61, //response to service 0x21
        0x01, // pid 1
        0x12, 0x12,0x12,0x12,
        0x23, 0xff, 0xff,0xff
    ];

    let frames = send(b);
    println!("{:x?}", frames);


    assert!(frames.len() == 2);
    assert!(frames[1] == [0x12, 0x23, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
}
