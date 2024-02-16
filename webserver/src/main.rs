const SERVER: Token = Token(0);

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
}

fn main() {
    println!("Hello, world!");
}

