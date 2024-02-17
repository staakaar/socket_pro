use log::error;
const SERVER: Token = Token(0);
const WEBROOT: &str = "/webroot";

struct WebServer {
    listening_socket: TcpListener,
    connections: HashMap<usize, TcpStream>,
    next_connection_id: usize,
}

impl WebServer {
    /** サーバーの初期化 */
    fn new(addr: &str) -> Result<Self, failure::Error> {
        let address = addr.parse()?;
        let listening_socket = TcpListener::bind(&address)?;
        Ok(WebServer {
            listening_socket,
            connections: HashMap::new(),
            next_connection_id: 1,
        })
    }

    /** サーバーを起動する */
    fn run(&mut self) -> Result<(), failure::Error> {
        let poll = Poll::new()?;

        poll.register(&self.listening_socket, SERVER, Ready::readable(), PollOpt::level())?;

        let mut events = Events::with_capacity(1024);
        let mut response = Vec::new();

        loop {
            match poll.poll(&mut events, None) {
                Ok(_) => {}
                Err(e) => {
                    error!("{}", e);
                    continue;
                }
            }
        }

        for event in &events {
            match event.token() {
                SERVER => {
                    let (stream, remote) = match self.listening_socket.accept() {
                        Ok(t) => t,
                        Err(e) => {
                            error!("{}", e);
                            continue;
                        }
                    }

                    debug!("Connection from {}", &remote);
                    self.register_connection(&poll, stream).unwrap_or_else(|e| error!("{}", e));
                }

                Token(conn_id) => {
                    self.http_handler(conn_id, event, &poll, &mut response).unwrap_or_else(|e| error!("{}", e));
                }
            }
        }
    }

    fn register_connection(&mut self, poll: &Poll, stream: TcpStream) -> Result<(), failure::Error> {
        let token = Token(self,next_connection_id);
        poll.register(&stream, token, Ready::readable(), PollOpt::edge())?;

        if self.connections.insert(self.next_connection_id, stream).is_some() {
            error!("Connection ID is already exist");
        }

        self.next_connection_id += 1;
        Ok(())
    }

    fn http_handler(&mut self, conn_id: usize, event: Event, poll: &Poll, response: &mut Vec<u8>) -> Result<(), failure::Error> {
        let stream = self.connections.get_mut(&conn_id).ok_or_else(|| failure::err_msg("Failed to get connnection."))?;
        if event.readiness().is_readable() {
            debug!("readable conn_id: {}", conn_id);
            let mut buffer = [0u8, 1024];
            let nbytes = stream.read(&mut buffer)?;

            if nbytes != 0 {
                *response = make_response(&buffer[..nbytes])?;
                poll.reregister(stream, Token(conn_id), Ready::writable, PollOpt::edge())?;
            } else {
                self.connections.remove(&conn_id);
            }
            Ok(())
        } else if event.readiness().is_writable() {
            debug!("writable conn_id: {}", conn_id);
            stream.write_all(response)?;
            self.connections.remove(&conn_id);
            Ok(())
        } else {
            Err(failure::err_msg("Undefined event"))
        }
    }

    fn make_response(buffer: &[u8]) -> Result<Vec<u8>, failure::Error> {
        let http_pattern = Regex::new(r"(.*)(.*) HTTP/1.([0-1])\r\n*")?;
        let captures = match http_pattern.captures(str::from_utf8(buffer)?) {
            Some(cap) => cap,
            None => {
                return create_msg_from_code(400, None);
            }
        };

        let method = captures[1].to_string();
        let path = format!(
            "{}{}{}",
            env::current_dir()?.display(),
            WEBROOT,
            &captures[2]
        );

        let _version = captures[3].to_string();

        if methos == "GET" {
            let file = match File::open(path) {
                Ok(file) => file,
                Err(_) => {
                    return create_msg_from_code(404, None);
                }
            }
            let mut reader = BufReader::new(file);
            let mut buf - Vec::new();
            reader.read_to_end(&mut buf)?;
            create_msg_from_code(200, Some(buf))
        } else {
            create_msg_from_code(501, None)
        }
    }

    fn create_msg_from_code(status_code: u16, msg: Option<Vec<u8>>) -> Result<Vec<u8>, failure::Error> {
        match status_code {
            200 => {
                let mut header = "HTTP/1.0 200 OK\r\n\ Server: mio webserver\r\n\r\n".to_string().into_bytes();

                if let Some(mut msg) = msg {
                    header.append(&mut msg);
                }
                Ok(header)
            }
            400 => Ok("HTTP/1.0 400 Bad Request\r\n\ Server: mio webserver\r\n\r\n".to_string().into_bytes();)
            404 => Ok("HTTP/1.0 404 Not Found\r\n\ Server: mio webserver\r\n\r\n".to_string().into_bytes();)
            501 => Ok("HTTP/1.0 501 Not Implemented\r\n\ Server: mio webserver\r\n\r\n".to_string().into_bytes();)
            _ => Err(failure::err_msg("Undefined status code."))
        }
    }
}

fn main() {
    env::set_var("RUST_LOG", "debug");
    env_logger::init();
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        error!("wrong number of arguments");
        process::exit(1);
    }

    let mut server = WebServer::new(&args[1]).unwrap_or_else(|e| {
        error!("{}", e);
        panic!();
    });

    server.run().unwrap_or_else(|e| {
        error!("{}", e);
        panic!();
    });
}

