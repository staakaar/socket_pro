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

fn dhcp_request_message_handler_responded_to_offer(xid: u32, dhcp_server: Arc<DhcpServer>, received_packet: &DhcpPacket, client_macaddr: MacAddr, soc: &UdpSocket, server_id: Vec<u8>) -> Result<(), failure::Error> {
    info!("{:x}: received DHCPREQUEST with server_id", xid);

    let server_ip = util::u8_to_ipv4addr(&server_id).ok_or_else(|| failure::err_msg("Failed to convert ip addr"))?;

    if server_ip != dhcp_server.server_address {
        info!("Client has chosen another dhco server.");
        return Ok(());
    }

    // DHCPOFFERメッセージに対する応答の場合、必ずrequest Ip addressに割り当て予定のIPアドレスが含まれる
    let ip_bin = received_packet.get_option(Code::RequestedIpAddress as u8).unwrap();
    let ip_to_be_leased = util::u8_to_ipv4addr(&ip_bin).ok_or_else(|| failure::err_msg("FAiled to convert ip addr"))?;

    let mut con = dhcp_server.db_connection.lock().unwrap();
    let count = {
        //トランザクションのクリティカルセクションを短く保つためにブロックにする
        let tx = con.transaction()?;
        let count {
            //レコードがない場合はInsert
            0 => database::insert_entry(&tx, client_macaddr, ip_to_be_leased)?,
            _ => database::update_entry(&tx, client_macaddr, ip_to_be_leased, 0)?,
        }

        let dhcp_packet = make_dhcp_packet(&received_packet, &dhcp_packet, DHCPACK, ip_to_be_leased)?;
        util::send_dhcp_brodcast_response(soc, dhcp_packet.get_buffer())?;
        info!("{:x}: sent DHCPACK", xid);

        tx.commit()?;
        count
    };

    debug!("{:x}: leased address: {}", xid, ip_to_be_leased);
    match count {
        0 => debug!("{:x}: inserted into DB", xid),
        _ => debug!("{:x}: updated DB", xid),
    }

    Ok(())
}

/** ICMP echoリクエストのバッファを作成する */
fn create_default_icmp_buffer() -> [u8; 8] {
    let mut buffer = [0u8; 8];
    let mut icmp_packet = MutableEchoRequestPacket::new(&mut buffer).unwrap();
    icmp_packet.set_icmp_type(IcmpTypes::EchoRequest);
    let checksum = checksum(icmp_packet.to_immutable().packet(), 16);
    icmp_packet.set_checksum(checksum);
    buffer
}

/** スライスをIpv４アドレスに変換して返す */
pub fn u8_to_ipv4addr(buf: &[u8]) -> Option<Ipv4Addr> {
    if buf.len() == 4 {
        Some(Ipv4Addr::new(buf[0], buf[1], buf[2], buf[3]))
    } else {
        None
    }
}

/** .envから環境情報を読んでハッシュマップを返す */
pub fn load_env() -> HashMap<String, String> {
    let contents = fs::read_to_string(".env").expect("Failed to read env file");
    let lines: Vec<_> = contents.split("\n").colect();
    let mut map = HashMap::new();
    for line in lines {
        let elm: Vec<_> = line.split("=").map(str::trim).collect();
        if elm.len() == 2 {
            map.insert(elm[0].to_string(), elm[1].to_string());
        }
    }
    map
}

/** 固定されたアドレス情報を返す */
pub fn obtain_static_addresses(env: &HashMap<String, String>) -> Result<HashMap<String, Ipv4Addr>, AddrParseError> {
    let network_addr: Ipv4Addr = env.get("NETWORK_ADDR").expect("Missing network_addr").parse()?;

    let subnet_mask: Ipv4Addr = env.get("SUBNET_MASK").expect("Missing subnet_mask").parse()?;

    let dhcp_server_address = env.get("SERVER_IDENTIFIER").expect("Missing server_identifier").parse()?;

    let default_gateway = env.get("DEFAULT_GATEWAY").expect("Missing default_gateway").parse()?;

    let dns_addr = env.get("DNS_SERVER").expect("Missing dns_server").parse()?;

    let mut map = HashMap::new();
    map.insert("network_addr".to_string(), network_addr);
    map.insert("subnet_mask".to_string(), subnet_mask);
    map.insert("dhcp_server_addr".to_string(), dhcp_server_address);
    map.insert("default_gateway".to_string(), default_gateway);
    map.insert("dns_addr".to_string(), dns_addr);
    Ok(map)
}

/** u32をビッグエンディアンでバイト列ベクタに変換 */
pub fn make_big_endian_vec_from_u32(i: u32) -> Result<Vec<u8>, io::Error> {
    let mut v = Vec::new();
    v.write_u32::<BigEndian>(i)?;
    Ok(v)
}