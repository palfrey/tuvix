use anyhow::Result;
use tuvix::Builder;

fn main() -> Result<()> {
    env_logger::init();
    let args: Vec<String> = std::env::args().collect();
    Builder::new(&args[1])?.build_in_chroot()?;
    Ok(())
}
