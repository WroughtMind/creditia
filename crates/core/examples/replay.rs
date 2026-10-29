//! Local replay utility; only its explicitly sanitized output belongs in Git.
#[path = "../tests/support/replay.rs"]
mod replay;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input = args.next().ok_or("usage: replay INPUT OUTPUT")?;
    let output = args.next().ok_or("usage: replay INPUT OUTPUT")?;
    let data: replay::Input = serde_json::from_slice(&std::fs::read(input)?)?;
    std::fs::write(output, serde_json::to_vec_pretty(&replay::replay(&data))?)?;
    Ok(())
}
