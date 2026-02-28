use std::env;
use std::fs::File;
use std::io::Read;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Pass rom path");
        return;
    }
    let mut file = File::open(&args[1]).unwrap();
    let mut header = [0; 16];
    file.read_exact(&mut header).unwrap();
    let mapper = (header[7] & 0xF0) | (header[6] >> 4);
    let prg_size = header[4] as usize * 16384;
    let chr_size = header[5] as usize * 8192;
    println!("File: {}", args[1]);
    println!("Mapper: {}", mapper);
    println!("PRG size: {}", prg_size);
    println!("CHR size: {}", chr_size);
}
