use async_std::net::UdpSocket;

pub async fn shoot(addr: String) {
    let socket = UdpSocket::bind("0.0.0.0:0").await;

    let rst = socket.unwrap().send_to(b"1", addr).await;
    let _ = rst.is_err();
}
