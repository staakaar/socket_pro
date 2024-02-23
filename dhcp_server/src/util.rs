pub fn is_ipaddr_available(target_ip: Ipv4Addr) -> Result<(), failure::Error> {
    let icmp_buf = create_default_icmp_buffer();

    let icmp_packet = EchoRequestPacket::new(&icmp_buf).unwrap();

    let (mut transport_sender, mut transport_receiver) = transport::transport_channel(1024, TransportChannelType::Layer4(Ipv4(IpNextHeaderProtocols::Icmp)))?;
    
    transport_sender.send_to(icmp_packet, IpAddr::V4(target_ip))?;
    let (sender, receiver) = mpsc::channel();

    // ICMP echo リクエストのリプライに対してタイムアウトを設定するため、スレッドを起動する このスレッドはEchoリプライを受信するまで残り続ける
    thread::spawn(move || {
        let mut iter = icmp_packet_iter(&mut transport_receiver);
        let (packet, _) = iter.next().unwrap();

        if packet.get_icmp_type() == IcmpTypes::EchoReply {
            match sender.send(true) {
                Err(_) => {
                    info!("icmp timeout");
                }
                _ => {
                    return;
                }
            }
        }
    });

    if receiver.recv_timeout(Duration::from_millis(200)).is_ok() {
        // 制限時間内にEchoリプライが届いた場合、IPアドレスは使われている
        let message = format!("ip addr already in use: {}", target_ip);
        warn!("{}", message);
        Err(failure::err_msg(message))
    } else {
        debug!("not receved reply within timeout");
        Ok(())
    }
}

pub fn send_dhcp_brodcast_response(soc: &UdpSocket, data: &[u8]) -> Result<(), failure::Error> {
    let destination: SocketAddr = "255.255.255.255.68".parse()?;
    soc.send_to(data, destination)?;
    Ok(())
}