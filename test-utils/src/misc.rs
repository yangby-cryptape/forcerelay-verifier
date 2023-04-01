use ckb_types::{packed, prelude::*};
use rand::{thread_rng, Rng as _};

pub fn random_hash() -> packed::Byte32 {
    let mut rng = thread_rng();
    let mut buf = [0u8; 32];
    rng.fill(&mut buf);
    buf.pack()
}

pub fn random_out_point() -> packed::OutPoint {
    let index: u32 = thread_rng().gen_range(1, 100);
    packed::OutPoint::new_builder()
        .tx_hash(random_hash())
        .index(index.pack())
        .build()
}

pub fn random_bytes() -> Vec<u8> {
    let mut rng = thread_rng();
    let len: usize = rng.gen_range(0, 64);
    let mut buf = vec![0u8; len];
    rng.fill(&mut buf[..]);
    buf
}
