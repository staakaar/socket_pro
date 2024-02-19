fn main() {
    env::set_var("RUST_LOG", "debug");
    env_logger::init();

    let server_socket = UdpSocket::bind("0.0.0.0:67").expect("Failed to bind socket");
    server_socket.set_broadcast(true).unwrap();

    let dhcp_server = Arc::new(DhcpServer::new().unwrap_or_else(|e| panic!("Failed to start dhcp server. {:?\}", e)));

    loop {
        let mut recv_buf = [0u8; 1024];
        match server_socket.rec_from(&mut recv_buf) {
            Ok((size, src)) => {
                debug!("received data from {}, size: {}", src, size);
                let transmission_socket = server_socket.try_clone().expect("Failed to create client socket");

                let cloned_dhcp_server = dhcp_server.clone();

                thread::spawn(move || {
                    if Some(dhcp_server) = DhcpPacket::new(recv_buf[..size].to_vec()) {

                        if dhcp_packet.get_op() != BOOTREQUEST {
                            return;
                        }
                        if let Err(e) = dhcp_handler(&dhcp_packet, &transmission_socket, cloned_dhcp_server) {
                            error!("{}", e);
                        }
                    }
                });
            }
            Err(e) => {
                error!("Cound not receive a datagram: {}", e);
            }
        }
    }
}
