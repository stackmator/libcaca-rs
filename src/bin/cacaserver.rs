//! Port of libcaca's `src/cacaserver.c`: telnet ASCII-art server.
//!
//! Reads a `.caca` stream from standard input and broadcasts every frame
//! as UTF-8 to telnet clients on port 51914 (`0xCACA`), negotiating like
//! the C version (echo, suppress-go-ahead, NAWS, ANSI cursor addressing).
//! Implemented on blocking stdin plus non-blocking [`TcpStream`]s, exactly
//! mirroring the C socket handling, backlog buffering and overflow reset.
//!
//! Two cleanups versus C: the server struct is properly initialised (the C
//! version reads uninitialised `malloc` memory for the NAWS size and the
//! socket count, advertising 0x0 here instead of garbage), and `SIGPIPE`
//! is ignored through `libc` on Unix.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use libcaca::Canvas;

// The C `BACKLOG` (1337) is passed to `listen()`; `std` manages the listen
// queue internally, so there is no equivalent constant here.
const INBUFFER: usize = 32;
const OUTBUFFER: usize = 300000;
const PORT: u16 = 0xCACA;

const ANSI_PREFIX: &[u8] = b"\x1b[1;1H\x1b[1;1H";
const ANSI_RESET: &[u8] = b"    \x1b[?1049h\x1b[?1049h";

const TELNET_COMMANDS: [&str; 16] = [
    "SE  ", "NOP ", "DM  ", "BRK ", "IP  ", "AO  ", "AYT ", "EC  ", "EL  ", "GA  ", "SB  ", "WILL",
    "WONT", "DO  ", "DONT", "IAC ",
];
const TELNET_OPTIONS: [&str; 37] = [
    "????", "ECHO", "????", "SUGH", "????", "STTS", "TIMK", "????", "????", "????", "????", "????",
    "????", "????", "????", "????", "????", "????", "????", "????", "????", "????", "????", "????",
    "TTYP", "????", "????", "????", "????", "????", "????", "NAWS", "TRSP", "RMFC", "LIMO", "????",
    "EVAR",
];

fn command_name(x: u8) -> &'static str {
    if x >= 240 {
        TELNET_COMMANDS[(x - 240) as usize]
    } else {
        "????"
    }
}

fn option_name(x: u8) -> &'static str {
    if (x as usize) < TELNET_OPTIONS.len() {
        TELNET_OPTIONS[x as usize]
    } else {
        "????"
    }
}

fn init_prefix(width: u16, height: u16) -> Vec<u8> {
    let mut prefix = b"\xff\xfb\x01\xff\xfb\x03\xff\xfd\x31\xff\x1f\xfa____\xff\xf0".to_vec();
    let pos = prefix.windows(4).position(|w| w == b"____").unwrap();
    prefix[pos] = (width >> 8) as u8;
    prefix[pos + 1] = (width & 0xff) as u8;
    prefix[pos + 2] = (height >> 8) as u8;
    prefix[pos + 3] = (height & 0xff) as u8;
    prefix.extend_from_slice(b"\x1b]2;caca for the network\x07\x1b[H\x1b[J");
    prefix
}

/// Outcome of flushing a client's backlog.
enum Backlog {
    /// The connection is dead.
    Drop,
    /// Backlog remains; the new frame was queued behind it.
    Pending,
    /// No backlog left; send the new frame directly.
    Empty,
}

struct Client {
    stream: TcpStream,
    ready: bool,
    inbuf: [u8; INBUFFER],
    inbytes: usize,
    outbuf: Vec<u8>,
    start: usize,
    stop: usize,
}

struct Server {
    canvas: Canvas,
    input: Vec<u8>,
    prefix: Vec<u8>,
    buffer: Vec<u8>,
    clients: Vec<Client>,
    listeners: Vec<TcpListener>,
}

/// A non-blocking write that retries on interrupts, like the C helper.
/// Returns the bytes written, or `None` on a real error (`WouldBlock`
/// yields `Some(0)` so callers can buffer the remainder).
fn nonblock_write(stream: &mut TcpStream, mut buf: &[u8]) -> Option<usize> {
    let mut total = 0;
    while !buf.is_empty() {
        match stream.write(buf) {
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(_) => return None,
            Ok(0) => break,
            Ok(n) => {
                total += n;
                buf = &buf[n..];
            }
        }
    }
    Some(total)
}

impl Server {
    fn send_data(&mut self, idx: usize) -> bool {
        // Returns true when the connection must be dropped.
        let buflen = self.buffer.len();

        // Listen to incoming data.
        loop {
            let mut byte = [0u8; 1];
            let ret = self.clients[idx].stream.read(&mut byte);
            let n = match ret {
                Ok(n) => n as isize,
                Err(_) => break,
            };
            if n <= 0 {
                break;
            }
            let c = &mut self.clients[idx];
            c.inbuf[c.inbytes] = byte[0];
            c.inbytes += 1;

            // Check for telnet sequences.
            if c.inbuf[0] == 0xff {
                if c.inbytes == 1 {
                    // Wait for more.
                } else if c.inbuf[1] == 0xfd || c.inbuf[1] == 0xfc {
                    if c.inbytes == 3 {
                        eprintln!(
                            "[{}] said: {:02x} {:02x} {:02x} ({} {} {})",
                            c.stream
                                .peer_addr()
                                .map(|a| a.to_string())
                                .unwrap_or_else(|_| "?".into()),
                            c.inbuf[0],
                            c.inbuf[1],
                            c.inbuf[2],
                            command_name(c.inbuf[0]),
                            command_name(c.inbuf[1]),
                            option_name(c.inbuf[2]),
                        );
                        // Just ignore, lol.
                        c.inbytes = 0;
                    }
                } else {
                    c.inbytes = 0;
                }
            } else if c.inbytes == 1 {
                if c.inbuf[0] == 0x03 {
                    eprintln!(
                        "[{}] pressed C-c",
                        c.stream
                            .peer_addr()
                            .map(|a| a.to_string())
                            .unwrap_or_else(|_| "?".into()),
                    );
                    return true; // User requested to quit.
                }
                c.inbytes = 0;
            }
            if c.inbytes >= INBUFFER {
                c.inbytes = 0;
            }
        }

        // Send the telnet initialisation commands.
        if !self.clients[idx].ready {
            let prefix = self.prefix.clone();
            match nonblock_write(&mut self.clients[idx].stream, &prefix) {
                None => return true,
                Some(n) if n < prefix.len() => return false,
                _ => {}
            }
            self.clients[idx].ready = true;
        }

        // No error, there's just nothing to send yet.
        if self.buffer.is_empty() {
            return false;
        }

        // If we have backlog, send the backlog.
        match self.flush_backlog(idx) {
            Backlog::Drop => return true,
            Backlog::Pending => return self.buffer_backlog(idx, buflen),
            Backlog::Empty => {}
        }

        // We no longer have backlog, send our new data.
        let c = &mut self.clients[idx];
        let prefix_len = ANSI_PREFIX.len();
        match nonblock_write(&mut c.stream, ANSI_PREFIX) {
            None => {
                eprintln!("[client] failed");
                return true;
            }
            Some(n) if n < prefix_len => {
                if prefix_len + buflen > OUTBUFFER {
                    c.outbuf[..ANSI_RESET.len()].copy_from_slice(ANSI_RESET);
                    c.start = 0;
                    c.stop = ANSI_RESET.len();
                    return false;
                }
                c.outbuf[..prefix_len - n].copy_from_slice(&ANSI_PREFIX[n..]);
                c.stop = prefix_len - n;
                c.outbuf[c.stop..c.stop + buflen].copy_from_slice(&self.buffer);
                c.stop += buflen;
                return false;
            }
            _ => {}
        }

        match nonblock_write(&mut c.stream, &self.buffer) {
            None => {
                eprintln!("[client] failed");
                return true;
            }
            Some(n) if n < buflen => {
                if buflen > OUTBUFFER {
                    c.outbuf[..ANSI_RESET.len()].copy_from_slice(ANSI_RESET);
                    c.start = 0;
                    c.stop = ANSI_RESET.len();
                    return false;
                }
                c.outbuf[..buflen - n].copy_from_slice(&self.buffer[n..]);
                c.stop = buflen - n;
                return false;
            }
            _ => {}
        }

        false
    }

    /// Send pending backlog. `Empty` means the new frame can go out now.
    fn flush_backlog(&mut self, idx: usize) -> Backlog {
        let (start, stop) = {
            let c = &self.clients[idx];
            (c.start, c.stop)
        };
        if start == stop {
            return Backlog::Empty;
        }
        let data = self.clients[idx].outbuf[start..stop].to_vec();
        match nonblock_write(&mut self.clients[idx].stream, &data) {
            None => {
                eprintln!("[client] failed");
                Backlog::Drop
            }
            Some(n) => {
                let c = &mut self.clients[idx];
                if n == stop - start {
                    // We got rid of the backlog!
                    c.start = 0;
                    c.stop = 0;
                    Backlog::Empty
                } else {
                    c.start += n;
                    Backlog::Pending
                }
            }
        }
    }

    /// Queue another frame behind existing backlog, resetting on overflow.
    fn buffer_backlog(&mut self, idx: usize, buflen: usize) -> bool {
        let c = &mut self.clients[idx];
        if c.stop - c.start + ANSI_PREFIX.len() + buflen > OUTBUFFER {
            // Overflow! Empty buffer and start again.
            c.outbuf[..ANSI_RESET.len()].copy_from_slice(ANSI_RESET);
            c.start = 0;
            c.stop = ANSI_RESET.len();
            return false;
        }
        if c.stop + ANSI_PREFIX.len() + buflen > OUTBUFFER {
            let pending = c.stop - c.start;
            c.outbuf.copy_within(c.start..c.stop, 0);
            c.stop = pending;
            c.start = 0;
        }
        c.outbuf[c.stop..c.stop + ANSI_PREFIX.len()].copy_from_slice(ANSI_PREFIX);
        c.stop += ANSI_PREFIX.len();
        c.outbuf[c.stop..c.stop + buflen].copy_from_slice(&self.buffer);
        c.stop += buflen;
        false
    }

    fn manage_connections(&mut self) {
        // Collect first to satisfy the borrow checker.
        let mut pending = Vec::new();
        for listener in &self.listeners {
            loop {
                match listener.accept() {
                    Err(_) => break,
                    Ok((stream, addr)) => pending.push((stream, addr.to_string())),
                }
            }
        }
        for (stream, addr) in pending {
            eprintln!("[new] connected from {addr}");
            let _ = stream.set_nonblocking(true);
            let client = Client {
                stream,
                ready: false,
                inbuf: [0; INBUFFER],
                inbytes: 0,
                outbuf: vec![0; OUTBUFFER],
                start: 0,
                stop: 0,
            };
            // If we already have data to send, send it to the new client.
            self.clients.push(client);
            let idx = self.clients.len() - 1;
            if self.send_data(idx) {
                eprintln!("[new] dropped connection");
                self.clients.pop();
            }
        }
    }
}

fn read_exact_1(stdin: &mut std::io::StdinLock<'_>) -> Option<u8> {
    let mut byte = [0u8; 1];
    loop {
        match stdin.read(&mut byte) {
            Ok(1) => return Some(byte[0]),
            Ok(_) => return None,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        }
    }
}

fn main() {
    #[cfg(unix)]
    unsafe {
        // Ignore SIGPIPE, like the C version.
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
    }

    let mut server = Server {
        canvas: Canvas::new(0, 0).unwrap(),
        input: Vec::new(),
        prefix: init_prefix(0, 0),
        buffer: Vec::new(),
        clients: Vec::new(),
        listeners: Vec::new(),
    };

    for host in ["0.0.0.0", "::"] {
        match TcpListener::bind((host, PORT)) {
            Err(_) => continue,
            Ok(listener) => {
                let _ = listener.set_nonblocking(true);
                eprintln!("listening on {}", listener.local_addr().unwrap());
                server.listeners.push(listener);
            }
        }
    }

    if server.listeners.is_empty() {
        eprintln!("Not listening");
        std::process::exit(-1);
    }

    eprintln!("initialised network, listening on port {}", PORT);

    let stdin = std::io::stdin();
    let mut lock = stdin.lock();

    // Main loop.
    loop {
        // Manage new connections.
        server.manage_connections();

        // Read data from stdin, keeping 12 bytes buffered.
        while server.input.len() < 12 {
            match read_exact_1(&mut lock) {
                Some(b) => server.input.push(b),
                // Like C, which ignores short reads and charges on.
                None => server.input.push(0),
            }
        }

        while server
            .canvas
            .import_from_memory(&server.input, "caca")
            .is_err()
        {
            server.input.remove(0);
            match read_exact_1(&mut lock) {
                Some(b) => server.input.push(b),
                None => server.input.push(0),
            }
        }

        loop {
            match server.canvas.import_from_memory(&server.input, "caca") {
                Err(_) => break,
                Ok(used) => {
                    if used > 0 {
                        server.input.drain(..used);
                        break;
                    }
                }
            }
            server.input.reserve(128);
            let mut chunk = [0u8; 128];
            match lock.read(&mut chunk) {
                Ok(0) | Err(_) => continue,
                Ok(n) => server.input.extend_from_slice(&chunk[..n]),
            }
        }

        // Get the ANSI representation and skip the trailing "\r\n".
        let data = server.canvas.export_to_memory("utf8cr").unwrap();
        let len = data.len().saturating_sub(2);
        server.buffer.clear();
        server.buffer.extend_from_slice(&data[..len]);

        let mut i = 0;
        while i < server.clients.len() {
            if server.send_data(i) {
                eprintln!("[client] dropped connection");
                server.clients.remove(i);
                continue;
            }
            i += 1;
        }
    }
}
