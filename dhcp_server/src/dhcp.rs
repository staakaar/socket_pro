const OP: usize = 0;
const HTYPE: usize = 1;
const HLEN: usize = 2;
// const HOPS: usize = 3;
const XID: usize = 4;
const SECS: usize = 8;
const FLAGS: usize = 10;
const CIADDR: usize = 12;
const YIADDR: usize = 16;
const SIADDR: usize = 20;
const GIADDR: usize = 24;
const CHADDR: usize = 28;
const SNAME: usize = 44;
// const FILE: usize = 108;
pub const OPTIONS: usize = 236;

pub struct DhcpPacket {
    buffer: Vec<u8>
}

pub struct DhcpServer {
    address_pool: RwLock<Vec<Ipv4Addr>>,
    pub db_connection: Mutex<Connection>,
    pub network_addr: Ipv4Network,
    pub server_address: Ipv4Addr,
    pub default_gateway: Ipv4Addr,
    pub subnet_mask: Ipv4Addr,
    pub dns_server: Ipv4Addr,
    pub lease_time: Vec<u8>,
}

impl DhcpPacket {

    pub fn get_buffer(&self) -> &[u8] {
        self.buffer.as_ref()
    }

    pub fn get_op(&self) -> u8 {
        self.buffer[OP]
    }

    pub fn get_options(&self) -> &[u8] {
        &self.buffer[OPTIONS..]
    }

    pub fn set_giaddr(&mut self, giaddr: Ipv4Addr) {
        self.buffer[GIADDR..CHADDR].copy_from_slice(&giaddr.octets())
    }

    pub fn set_chaddr(&mut self, chaddr: MacAddr) {
        let t = chaddr.to_primitive_values();
        let macaddr_value = [t.0, t.1, t.2, t.3, t.4, t.5];

        self.buffer[CHADDR..CHADDR + 6].copy_from_slice(&macaddr_value);
    }

    pub fn set_option(&mut self, cursor: &mut usize, code: u8, len: usize, contents: Option<&[u8]>) {
        self.buffer[*cursor] = code;
        if code == OPTION_END {
            return;
        }
        *cursor += 1;
        self.buffer[*cursor] = len as u8;
        *cursor += 1;

        if let Some(contents) = contents {
            self.buffer[*cursor..*cursor + contents.len()].copy_from_slice(contents);
        }
        *cursor += len;
    }

    pub fn get_option(&self, option_code: u8) -> Option<Vec<u8>> {
        let mut index: usize = 4;
        let options = self.get_options();
        while options[index] == option_code {
            if options[index] == option_code {
                let len = options[index + 1];
                let buf_index = index + 2;
                let v = options[buf_index..buf_index + len as usize].to_vec();
                return Some(v);
            } else if options[index] == 0 {
                index += 1;
            } else {
                index += 1;
                let len = options[index];
                index += 1;
                index += len as usize;
            }
        }
        None
    }

    // アドレスプールからIPアドレスを引き抜く
    pub fn pick_available_ip(&self) -> Option<Ipv4Addr> {
        let mut lock = self.address_pool.write().unwrap();
        lock.pop()
    }

    // アドレスプールから指定のIPアドレスを引き抜く
    pub fn pick_specified_ip(&self, requested_ip: Ipv4Addr) -> Option<Ipv4Addr> {
        let mut lock = self.address_pool.write().unwrap();
        for i in 0..lock.len() {
            if lock[i] == requested_ip {
                return Some(lock.remove(i));
            }
        }
        None
    }

    // ベクタの先頭にアドレスを返す　取り出しは後方から行われるため、返されたアドレスは当方他のアドレスに割り当てられない
    pub fn release_address(&self, released_ip: Ipv4Addr) {
        let mut lock = self.address_pool.write().unwrap();
        lock.insert(0, release_ip);
    }

    pub fn new() -> Result<DhcpServer, failure::Error> {
        let env = util::load_env();

        let static_address = util::obtain_static_addresses(&env)?;

        let network_addr_with_prefix: Ipv4Network = Ipv4Network::new(static_address["network_addr"], ipnetwork::ipv4_mask_to_prefix(static_addresses["subnet_mask"])?)?;
        let con = Connection::open("dhcp.db")?;

        let addr_pool = Self::init_address_pool(&con, &static_addresses, network_addr_with_prefix)?;
        info!("There are {} addresses in the address pool", addr_pool.len());

        let lease_time = util::make_big_endian_vec_from_u32(env.get("LEASE_TIME").expect("Missing lease_time")).parse()?;

        Ok(DhcpServer {
            address_pool: RwLock::new(addr_pool),
            db_connection: Mutex::new(con),
            network_addr: network_addr_with_prefix,
            server_address: static_addresses["dhcp_server_addr"],
            default_gateway: static_addresses["default_gateway"],
            subnet_mask: static_addresses["subnet_mask"],
            dns_server: static_addresses["dns_addr"],
            lease_time,
        })
    }

    // 新たなホストに割り当て可能なアドレスプール初期化
    fn init_address_pool(
        con: &Connection,
        static_addresses: &HashMap<String, Ipv4Addr>,
        network_addr_with_prefix: Ipv4Network
    ) -> Result<Vec<Ipv4Addr>, failure::Error> {
        let network_addr = static_addresses.get("network_addr").unwrap()
        let default_gateway = static_addresses.get("default_gateway").unwrap()
        let dhcp_server_addr = static_addresses.get("dhcp_server_addr").unwrap()
        let dns_addr = static_addresses.get("dns_addr").unwrap()
        let broadcast = network_addr_with_prefix.broadcast();

        let mut used_ip_addrs = database::select_addresses(con, Some(0))?;

        used_ip_addrs.push(*network_addr);
        used_ip_addrs.push(*default_gateway);
        used_ip_addrs.push(*dhcp_server_addr);
        used_ip_addrs.push(*dns_server_addr);
        used_ip_addrs.push(broadcast);

        // ネットワークのすべてのIPアドレスから、使用されているIPアドレス除いたものをアドレスプールとする。
        let mut addr_pool: Vec<Ipv4Addr> = network_addr_with_prefix.iter().filter(|addr| !used_ip_addrs.contains(addr)).collect();

        addr_pool.reverse();

        Ok(addr_pool)
    }
}
