use std::{io, fs};
use std::io::{Read, Write};

fn main() -> Result<(), &'static str>{
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout.write("?\0\n".as_bytes()).map_err(|_| "couldn't write to stdout")?;
    let mut buf = vec![];
    stdout.flush().map_err(|_| "couldn't flush stdout")?;
    let mut file = fs::File::create("bad_log2.txt").map_err(|_| "couldn't open file")?;
    stdin.read(&mut buf).map_err(|_| "couldn't read")?;
    write!(file, "{:?}", buf).map_err(|_| "couldn't write to file")?;
    file.flush().map_err(|_| "couldn't flush file")?;
    stdin.read(&mut buf).map_err(|_| "couldn't read2")?;
    write!(file, "{:?}", buf).map_err(|_| "couldn't write to file2")?;
    stdout.write("\n".as_bytes()).map_err(|_| "couldn't write to stdout")?;
    Ok(())
}
