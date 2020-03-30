//! Provides functionality to connect with radare2.
//!
//! Please check crate level documentation for more details and example.

use reqwest;

use libc;
use std::process::Command;
use std::process::Stdio;
use std::process;
use std::env;
use std::str;
use std::path::Path;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::net::{TcpStream, ToSocketAddrs, SocketAddr};
use std::thread;
use std::sync::mpsc;
use std::sync::Arc;

use serde_json;
use serde_json::Value;
use serde_json::Error;

/// File descriptors to the parent r2 process.
pub struct R2PipeLang {
    read: BufReader<File>,
    write: File,
}

/// File descriptors to the parent r2 process.
pub struct R2PipeLangDuplex {
    duplex: BufReader<File>,
}

/// Stores descriptors to the spawned r2 process.
pub struct R2PipeSpawn {
    read: BufReader<process::ChildStdout>,
    write: process::ChildStdin,
}

/// Stores the socket address of the r2 process.
pub struct R2PipeTcp {
    socket_addr: SocketAddr,
}

pub struct R2PipeHttp {
    host: String,
}

/// Stores thread metadata
/// It stores both a sending and receiving end to the thread, allowing convenient interaction
/// So we can send commands using R2PipeThread::send() and fetch outputs using R2PipeThread::recv()
pub struct R2PipeThread {
	r2recv: mpsc::Receiver<String>,
	r2send: mpsc::Sender<String>,
	pub id: u16,
	pub handle: thread::JoinHandle<()>,
}

#[derive(Default, Clone)]
pub struct R2PipeSpawnOptions {
    pub exepath: String,
    pub args: Vec<&'static str>,
}

/// Provides abstraction between the three invocation methods.
pub enum R2Pipe {
    Pipe(R2PipeSpawn),
    Lang(R2PipeLang),
    LangDuplex(R2PipeLangDuplex),
    Tcp(R2PipeTcp),
    Http(R2PipeHttp),
}

fn atoi(k: &str) -> i32 {
    match k.parse::<i32>() {
        Ok(val) => val,
        Err(_) => -1,
    }
}

fn getenv(k: &str) -> i32 {
    match env::var(k) {
        Ok(val) => atoi(&val),
        Err(_) => -1,
    }
}

fn process_result(res: Vec<u8>) -> Result<String, String> {
    let len = res.len();
    let out = if len > 0 {
        let res_without_zero = &res[..len - 1];
        if let Ok(utf8str) = str::from_utf8(res_without_zero) {
            String::from(utf8str.trim())
        } else {
            return Err("Failed".to_owned());
        }
    } else {
        "".to_owned()
    };
    Ok(out)
}

#[macro_export]
macro_rules! open_pipe {
	() => {
            R2Pipe::open(),
        };
	($x: expr) => {
		match $x {
			Some(path) => R2Pipe::spawn(path, None),
			None => R2Pipe::open(),
		}
	};
	($x: expr, $y: expr) => {
		match $x $y {
			Some(path, opts) => R2Pipe::spawn(path, opts),
			(None, None) => R2Pipe::open(),
		}
	}
}

impl R2Pipe {
    #[cfg(not(windows))]
    pub fn open() -> Result<R2Pipe, &'static str> {
        use std::os::unix::io::FromRawFd;

        let (f_in, f_out) = match R2Pipe::in_session() {
            Some(x) => x,
            None => return Err("Pipe not open. Please run from r2"),
        };
        let res = unsafe {
            // dup file descriptors to avoid from_raw_fd ownership issue
            let (d_in, d_out) = (libc::dup(f_in), libc::dup(f_out));
            R2PipeLang {
                read: BufReader::new(File::from_raw_fd(d_in)),
                write: File::from_raw_fd(d_out),
            }
        };
        Ok(R2Pipe::Lang(res))
    }

    #[cfg(windows)]
    pub fn open() -> Result<R2Pipe, &'static str> {
        /*
        // We steal the pipe handle from STDIO because of a handle ownership issue
        //  error we're dodging: https://stackoverflow.com/questions/14883547
        //TODO: Investigate "CONIN$" "CONOUT$" 
        use std::os::windows::io::FromRawHandle;
        use std::ptr;
        use std::ffi::CString;
        use winapi::um::{processenv, errhandlingapi::GetLastError, fileapi, handleapi, processthreadsapi, winbase, winnt};
        let path = CString::new(R2Pipe::in_windows_session()
            .ok_or("Pipe not open, Please run from r2")?
        ).unwrap();
        let stdout = unsafe{processenv::GetStdHandle(winbase::STD_OUTPUT_HANDLE)};
        
        // 64 is way more than enough for the pipe path
        const PATH_SIZE: usize = 64;
        let mut observed_path = vec![0u8; PATH_SIZE];
        const FILE_NAME_OPENED: winapi::shared::minwindef::DWORD = 0x8;
        let err1 = 0u8;// unsafe{GetLastError()};
        let observed_path_len = unsafe{fileapi::GetFinalPathNameByHandleA(
            stdout,
            observed_path.as_mut_ptr() as _,
            PATH_SIZE as u32 - 1, // -1 because the winapi doesn't count null ptr!
            FILE_NAME_OPENED
        )};
        let err2 = unsafe{GetLastError()};
        println!("WINERRORS: {} {}", err1, err2);
        observed_path.truncate(observed_path_len as usize);
        if observed_path != path.into_bytes(){
            // println!("! echo {:?}", CString::new(observed_path));
            // println!("Wait what even is my stdout {:?}.", stdout);
            return Err("R2Pipe env variable invalid. Please don't manually set R2PIPE_PATH.");
        }

        let mut new_handle = ptr::null_mut();
        unsafe{
            let process_pseudohandle = processthreadsapi::GetCurrentProcess();
            if 0!=handleapi::DuplicateHandle(
                process_pseudohandle,
                stdout,
                process_pseudohandle,
                &mut new_handle,
                winnt::GENERIC_READ | winnt::GENERIC_WRITE,
                0,
                0
            ){
                return Err("Couldn't duplicate R2Pipe handle");
            }
        }
        
        Ok(R2Pipe::Lang(R2PipeLang{
            read: BufReader::new(unsafe{File::from_raw_handle(new_handle)}),
            write: None
        }))
        */
        use std::os::windows::io::FromRawHandle;
        use std::ptr;
        use std::ffi::CString;
        use winapi::um::{fileapi, namedpipeapi, errhandlingapi::GetLastError, winnt, handleapi, winbase};
        use winapi::shared::winerror;
        let f = CString::new(R2Pipe::in_windows_session()
            .ok_or("Pipe not open, Please run from r2")?
        ).unwrap();
        let res = unsafe{
            let mut pipe = handleapi::INVALID_HANDLE_VALUE;
            while pipe==handleapi::INVALID_HANDLE_VALUE{
                pipe = fileapi::CreateFileA(
                    f.as_ptr(),
                    winnt::GENERIC_READ | winnt::GENERIC_WRITE,
                    0,
                    ptr::null_mut(),
                    fileapi::OPEN_EXISTING,
                    0,
                    ptr::null_mut()
                );
                if pipe == handleapi::INVALID_HANDLE_VALUE {
                    if GetLastError()!=winerror::ERROR_PIPE_BUSY{
                        return Err("Couldn't open R2Pipe. Please don't set R2PIPE_PATH manually.");
                    }
                }
                if 0==winbase::WaitNamedPipeA(f.as_ptr(), 20000){
                    return Err("Couldn't open R2Pipe. 20 second timeout on ERROR_PIPE_BUSY.");
                }
                
            }
            let mut dw_mode = winbase::PIPE_READMODE_MESSAGE;
            let success = namedpipeapi::SetNamedPipeHandleState(
                pipe,
                &mut dw_mode,
                ptr::null_mut(),
                ptr::null_mut()
            );
            if success == 0 {
                return Err("winapi's SetNamedPipeHandleState failed");
            }
            R2PipeLangDuplex{
                duplex: BufReader::new(File::from_raw_handle(pipe))
            }
        };
        Ok(R2Pipe::LangDuplex(res))
    }

    pub fn cmd(&mut self, cmd: &str) -> Result<String, String> {
        match *self {
            R2Pipe::Pipe(ref mut x) => x.cmd(cmd.trim()),
            R2Pipe::Lang(ref mut x) => x.cmd(cmd.trim()),
            R2Pipe::LangDuplex(ref mut x) => x.cmd(cmd.trim()),
            R2Pipe::Tcp(ref mut x) => x.cmd(cmd.trim()),
            R2Pipe::Http(ref mut x) => x.cmd(cmd.trim()),
        }
    }

    pub fn cmdj(&mut self, cmd: &str) -> Result<Value, String> {
        match *self {
            R2Pipe::Pipe(ref mut x) => x.cmdj(cmd.trim()),
            R2Pipe::Lang(ref mut x) => x.cmdj(cmd.trim()),
            R2Pipe::LangDuplex(ref mut x) => x.cmdj(cmd.trim()),
            R2Pipe::Tcp(ref mut x) => x.cmdj(cmd.trim()),
            R2Pipe::Http(ref mut x) => x.cmdj(cmd.trim()),
        }
    }

    pub fn close(&mut self) {
        match *self {
            R2Pipe::Pipe(ref mut x) => x.close(),
            R2Pipe::Lang(ref mut x) => x.close(),
            R2Pipe::LangDuplex(ref mut x) => x.close(),
            R2Pipe::Tcp(ref mut x) => x.close(),
            R2Pipe::Http(ref mut x) => x.close(),
        }
    }

    pub fn in_session() -> Option<(i32, i32)> {
        let f_in = getenv("R2PIPE_IN");
        let f_out = getenv("R2PIPE_OUT");
        if f_in < 0 || f_out < 0 {
            return None;
        }
        Some((f_in, f_out))
    }

    #[cfg(windows)]
    pub fn in_windows_session() -> Option<String> {
        env::var("R2PIPE_PATH").ok()
    }

    /// Creates a new R2PipeSpawn.
    pub fn spawn<T: AsRef<str>>(name: T, opts: Option<R2PipeSpawnOptions>) -> Result<R2Pipe, &'static str> {
        if name.as_ref() == "" {
            if let Some(_) = R2Pipe::in_session() {
                return R2Pipe::open();
            }
        }

        let exepath = match opts {
            Some(ref opt) => opt.exepath.clone(),
            _ => "r2".to_owned(),
        };
        let args = match opts {
            Some(ref opt) => opt.args.clone(),
            _ => vec![],
        };
        let path = Path::new(name.as_ref());
        let child = match Command::new(exepath)
            .arg("-q0")
            .args(&args)
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn() {
            Ok(c) => c,
            Err(_) => return Err("Unable to spawn r2."),
        };

        let sin = child.stdin.unwrap();
        let mut sout = child.stdout.unwrap();

        // flush out the initial null byte.
        let mut w = [0; 1];
        sout.read(&mut w).unwrap();

        let res = R2PipeSpawn {
            read: BufReader::new(sout),
            write: sin,
        };

        Ok(R2Pipe::Pipe(res))
    }

    /// Creates a new R2PipeTcp
    pub fn tcp<A: ToSocketAddrs>(addr: A) -> Result<R2Pipe, &'static str> {
        // use `connect` to figure out which socket address works
        let stream = try!(TcpStream::connect(addr).map_err(|_| "Unable to connect TCP stream"));
        let addr = try!(stream.peer_addr().map_err(|_| "Unable to get peer address"));
        Ok(R2Pipe::Tcp(R2PipeTcp { socket_addr: addr }))
    }

    /// Creates a new R2PipeHttp
    pub fn http(host: &str) -> Result<R2Pipe, &'static str> {
        Ok(R2Pipe::Http(R2PipeHttp { host: host.to_string() }))
    }
	
	/// Creates new pipe threads
	/// First two arguments for R2Pipe::threads() are the same as for R2Pipe::spawn() but inside vectors
	/// Third and last argument is an option to a callback function
	/// The callback function takes two Arguments: Thread ID and r2pipe output
	pub fn threads(names: Vec<&'static str>, opts: Vec<Option<R2PipeSpawnOptions>>,
	callback: Option<Arc<Fn(u16, String)+Sync+Send>>)
	-> Result<Vec<R2PipeThread>, &'static str> {
		if names.len() != opts.len() { 
			return Err("Please provide 2 Vectors of the same size for names and options");
		}

		let mut pipes = Vec::new();

		for n in 0..names.len() {
			let (htx, rx) = mpsc::channel();
			let (tx, hrx) = mpsc::channel();
			let name = names[n];
			let opt = opts[n].clone();
			let cb = callback.clone();
			let t = thread::spawn(move || {
				let mut r2 = R2Pipe::spawn(name, opt).unwrap();
				loop {
					let cmd = String::from(hrx.recv().unwrap());
					if cmd == "q" { break; }
					let res = r2.cmdj(&cmd).unwrap().to_string();
					htx.send(res.clone()).unwrap();
					if let Some(cbs) = cb.clone() { thread::spawn(move || { cbs(n as u16, res);	}); };
				}
			});
			pipes.push(R2PipeThread { r2recv: rx, r2send: tx, id: n as u16, handle: t });
		}
		Ok(pipes)
	}	
}

impl R2PipeThread {
	pub fn send(&self, cmd: String) -> Result<(), String> {
		if let Ok(val) = self.r2send.send(cmd) { return Ok(val) };
		Err("Send Error!".to_string())
	}

	pub fn recv(&self, block: bool) -> Result<String, String> {
		match block {
			true => if let Ok(val) = self.r2recv.recv() { return Ok(val) },
			false => if let Ok(val) = self.r2recv.try_recv() { return Ok(val) }
		};
		Err("Receive Error!".to_string())
	}
}

impl R2PipeSpawn {
    pub fn cmd(&mut self, cmd: &str) -> Result<String, String> {
        let cmd_ = cmd.to_owned() + "\n";
        if let Err(e) = self.write.write(cmd_.as_bytes()) {
            return Err(e.to_string());
        }

        let mut res: Vec<u8> = Vec::new();
        if let Err(e) = self.read.read_until(0u8, &mut res) {
            return Err(e.to_string());
        }
        process_result(res)
    }

    pub fn cmdj(&mut self, cmd: &str) -> Result<Value, String> {
        if let Ok(res) = self.cmd(cmd) {
            if res == "" {
                return Err("Empty JSON".to_string());
            }

            let v: Result<Value, Error> = serde_json::from_str(&res);
            if v.is_ok() {
                Ok(v.unwrap())
            } else {
                v.map_err(|e| e.to_string())
            }
        } else {
            Err("oops cmd".to_string())
        }
    }

    pub fn close(&mut self) {
        let _ = self.cmd("q!");
    }
}

impl R2PipeLang {
    pub fn cmd(&mut self, cmd: &str) -> Result<String, String> {
        self.write.write(cmd.as_bytes()).unwrap();
        let mut res: Vec<u8> = Vec::new();
        self.read.read_until(0u8, &mut res).unwrap();
        process_result(res)
    }

    pub fn cmdj(&mut self, cmd: &str) -> Result<Value, String> {
        let res = try!(self.cmd(cmd));

        let v: Result<Value, Error> = serde_json::from_str(&res);
        if v.is_ok() {
            Ok(v.unwrap())
        } else {
            v.map_err(|e| e.to_string())
        }
    }

    pub fn close(&mut self) {
        // self.read.close();
        // self.write.close();
    }
}

impl R2PipeLangDuplex {
    pub fn cmd(&mut self, cmd: &str) -> Result<String, String> {
        self.duplex.get_mut().write(cmd.as_bytes()).unwrap();
        let mut res: Vec<u8> = Vec::new();
        self.duplex.read_until(0u8, &mut res).unwrap();
        process_result(res)
    }

    pub fn cmdj(&mut self, cmd: &str) -> Result<Value, String> {
        let res = self.cmd(cmd)?;

        let v = serde_json::from_str(&res);
        v.map_err(|e| e.to_string())
    }

    pub fn close(&mut self) {
        // self.duplex.close();
    }
}

impl R2PipeHttp {

    pub fn cmd(&mut self, cmd: &str) -> Result<String, String> {
        let url = format!("http://{}/cmd/{}", self.host, cmd);
        let res = reqwest::get(&url).unwrap();
        let bytes = res.bytes()
            .into_iter()
            .filter_map(|e| e.ok())
            .collect::<Vec<_>>();
        str::from_utf8(bytes.as_slice())
            .map(|s| s.to_string())
            .map_err(|err| err.to_string())
    }

    pub fn cmdj(&mut self, cmd: &str) -> Result<Value ,String> {
        let res = try!(self.cmd(cmd));
        serde_json::from_str(&res)
            .map_err(|e| format!("Unable to parse json: {}", e))
    }

    pub fn close(&mut self) {}
}

impl R2PipeTcp {
    pub fn cmd(&mut self, cmd: &str) -> Result<String, String> {
        let mut stream = try!(TcpStream::connect(self.socket_addr)
                              .map_err(|e| format!("Unable to connect TCP stream: {}", e)));
        try!(stream.write_all(cmd.as_bytes())
             .map_err(|e| format!("Unable to write to TCP stream: {}", e)));
        let mut res: Vec<u8> = Vec::new();
        try!(stream.read_to_end(&mut res)
             .map_err(|e| format!("Unable to read from TCP stream: {}", e)));
        res.push(0);
        process_result(res)
    }

    pub fn cmdj(&mut self, cmd: &str) -> Result<Value, String> {
        let res = try!(self.cmd(cmd));
        serde_json::from_str(&res).map_err(|e| format!("Unable to parse json: {}", e))
    }

    pub fn close(&mut self) {}
}
