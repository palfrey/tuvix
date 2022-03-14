use anyhow::Result;
use tuvix::Builder;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    Builder::new(&args[1])?.build_in_chroot()?;
    Ok(())
}
