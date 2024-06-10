use bytes::{BytesMut};

fn main() {
    let mut buf = BytesMut::with_capacity(1024 * 1024);
    buf.resize(buf.capacity(), 0);
    // println!("{}", buf.capacity());
    println!("{:?}", &buf.iter().as_slice()[..4]);
}