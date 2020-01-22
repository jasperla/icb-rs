use std::fs::File;
use std::io::prelude::*;

/// Quick-print function which writes its contents to `/tmp/q.log` (truncating
/// the file if it already exists).
/// For example:
///     let vector = vec![1, 2, 3];
///     q("v", &vector);
pub fn q<T: std::fmt::Debug>(msg: &str, thing: &T) -> std::io::Result<()> {
    let mut file = File::create("/tmp/q.log")?;
    file.write_all(format!("{}: {:?}\n", msg, thing).as_bytes())?;
    Ok(())
}
