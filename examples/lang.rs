extern crate r2pipe;
extern crate serde_json;
extern crate winapi;
use r2pipe::R2Pipe;
use winapi::um::errhandlingapi::GetLastError;

fn main() -> Result<(), &'static str>{
    print!("?");
    print!("?");
    print!("?");
    let mut r2 = match R2Pipe::open(){
        Ok(r2) => r2,
        Err(err) => {
            let winerror = unsafe{GetLastError()};
            let mut f = std::fs::File::create("bad_log.txt").unwrap();
            use std::io::Write;
            writeln!(f, "ERR: {} {} {}", winerror, R2Pipe::in_windows_session().unwrap(), err).unwrap();
            use std::process;
            process::exit(1);
            println!("r2pipe lang example: launching r2 around us");
            let cmd = format!("#!pipe {}", std::env::args().next().unwrap());
            process::Command::new("radare2")
                .args(&["-qc", &cmd, "--"])
                .spawn()
                .expect("couldn't run radare2")
                .wait()
                .expect("failed to wait on r2");
            process::exit(0)
        }
    };
    
    r2.cmd("! echo Hello from Rust!").unwrap();
    
    Ok(())
}