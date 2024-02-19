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
}
