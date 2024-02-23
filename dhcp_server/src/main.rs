enum Code {
    MessageType = 53,
    IPAddressLeaseTime = 51,
    ServerIdentifier = 54
    RequestedIpAddress = 50,
    SubnetMask = 1,
    Router = 3,
    DNS = 6,
    End = 255,
}

const DHCPDISCOVER: u8 = 1;
const DHCPOFFER: u8 = 2;
const DHCPREQUEST: u8 = 3;
const DHCPPACK: u8 = 5;
const DHCPNAK: u8 = 6;
const DHCPRELEASE: u8 = 7;

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

fn make_dhcp_packet(
    received_packet: &DhcpPacket,
    dhcp_server: &Arc<DhcpServer>,
    message_type: u8,
    ip_to_be_leased: Ipv4Addr
) -> Result<DhcpPacket, failure::Error> {
    /** パケット本体となるバッファ */
    let buffer = vec![0u8; DHCP_SIZE];
    let mut dhcp_packet = DhcpPacket::new(buffer).unwrap();

    /** 各種フィールドの設定 */
    dhcp_server.set_op(BOOTREPLY);
    dhcp_server.set_htype(HTYPE_ETHER);
    dhcp_packet.set_hlen(6);
    dhcp_server.set_xid(received_packet.get_xid())

    if message_type == DHCPACK {
        dhcp_packet.set_ciaddr(received_packet.get_ciaddr())
    }
    dhcp_packet.set_yiaddr(ip_to_be_leased);
    dhcp_packet.set_flags(received_packet.get_flags());
    dhcp_server.set_giaddr(received_packet.get_giaddr());
    dhcp_server.set_chaddr(received_packet.get_chaddr());

    /** 各種オプションの設定 */
    let mut cursor = dhcp::OPTIONS;
    dhcp_packet.set_magic_cookie(&mut cursor);
    dhcp_packet.set_option(&mut cursor, Code::MessageType as u8, 1, Some(&[message_type]));
    dhcp_packet.set_option(&mut cursor, Code::IPAddressLeaseTime as u8, 4, Some(&dhcp_server.lease_time));
    dhcp_packet.set_option(&mut cursor, Code::ServerIdentifier as u8, 4, Some(&dhcp_server.server_address.octets()));
    dhcp_packet.set_option(&mut cursor, Code::SubnetMask as u8, 4, Some(&dhcp_server.subnet_mask.octets()));
    dhcp_packet.set_option(&mut cursor, Code::Router as u8, 4, Some(&dhcp_server.default_gateway.octets()));
    dhcp_packet.set_option(&mut cursor, Code::DNS as u8, 4, Some(&dhcp_server.dns_server.octets()));
    dhcp_packet.set_option(&mut cursor, Code:: as u8, 0, None);
    Ok(dhcp_packet)
}

fn dhcp_handler(packet: &DhcpPacket, soc: &UdpSocket, dhcp_server: Arc<DhcpServer>) -> Result<(), failure::Error> {
    let message = packet.get_option(Code::MessageType as u8).ok_or_else(|| failure::err_msg("spacified option was not found"))?;
    let message_type = message[0];
    let transaction_id = BigEndian::read_u32(packet.get_xid());
    let client_macaddr = packet.get_chaddr();

    match message_type {
        DHCPDISCOVER => dhcp_discover_message_handler(transaction_id, dhcp_server, &packet, soc),
        DHCPREQUEST => match packet.get_option(Code::ServerIdentifier as u8) {
            Some(server_id) => dhcp_request_message_handler_responded_to_offer(transaction_id, dhcp_server, &packet, client_macaddr, soc, server_id),
            None => dhcp_request_message_handler_to_reallocate(transaction_id, dhcp_server, &packet, client_macaddr, soc),
        },
        DHCPRELEASE => {
            dhcp_release_message_handler(transaction_id, dhcp_server, &packet, client_macaddr)
        }
        _ => {
            let msg = format!("{:x}: received unimplemented message, message_type:{}", transaction_id, message_type);
            Err(failure::err_msg(msg))
        }
    }
}

fn dhcp_discover_message_handler(xid: u32, dhcp_server: Arc<DhcpServer>, received_packet: &DhcpPacket, soc: &UdpSocket) -> Result<(), failure::Error> {
    info!("{:x}: received DHCPDISCOVER", xid);
    let ip_to_be_leased = select_lease_ip(&dhcp_server, &received_packet)?;
    let dhcp_packet = make_dhcp_packet(&received, &dhcp_server, DHCPOFFER, ip_to_be_leased)?;
    util::send_dhcp_brodcast_response(soc, dhcp_packet.get_buffer())?;
    info!("{:x}: sent DHCPOFFER", xid);
    Ok(())
}

fn select_lease_ip(dhcp_server: &Arc<DhcpServer>, received_packet: &DhcpPacket) -> Result<Ipv4Addr, failure::Error> {
    {
        let con = dhcp_server.db_connection.lock().unwrap();
        if let Some(ip_from_used) = database::select_entry(&con, received_packet.get_chaddr())? {
            // IPアドレスが重複していないか
            // .envに記載されたネットワークアドレスの変更があった時のために、現在のネットワークに含まれているかを合わせて確認する
            if dhcp_server.network_addr.contains(ip_from_used) && util::is_ipaddr_available(ip_from_used).is_ok() {
                return Ok(ip_from_used);
            }
        }
    }

    // Request Ip Addrオプションがあり、利用可能ならばそのIPアドレスを返却
    if let Some(ip_to_be_leased) = obtain_avaliable_ip_from_requested_option(dhcp_server, &received_packet) {
        return Ok(ip_to_be_leased)
    }

    // アドレスプールからの取得
    while let Some(ip_addr) = dhcp_server.pick_avaliable_ip() {
        if util::is_ipaddr_avaliable(ip_addr).is_ok() {
            return Ok(ip_addr);
        }
    }
    // 利用できるIPアドレスが取得できなかった場合
    Err(failure::err_msg("Cloud not obtain avaliable ip address."))
}

fn obtain_avaliable_ip_from_requested_option(dhcp_server &Arc<DhcpServer>, received_packet: &DhcpPacket) -> Option<Ipv4Addr> {
    let ip = received_packet.get_option(Code::RequestedIpAddress as u8)?;

    let request_ip = util::u8_to_ipv$addr(&ip)?;
    let ip_from_pool = dhcp_server.pick_avaliable_ip(request_ip)?;

    if util::is_ipaddr_available(ip_from_pool).is_ok() {
        return Some(requested_ip);
    }
    None
}