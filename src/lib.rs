use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use starlark::environment::{Globals, GlobalsBuilder, Module};
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::syntax::{AstModule, Dialect};
use starlark::values::none::NoneType;
use starlark::values::AnyLifetime;
use std::collections::VecDeque;
use std::path::Path;
use std::process::Command;
use std::{env, fs};

#[derive(AnyLifetime)]
struct Info {
    pub hash_path: String,
}

fn check_hash_for_bytes(contents: &[u8], expected_hash: &str) -> Result<(), String> {
    let mut hasher = Sha256::new();
    hasher.update(&contents);
    let hash = hasher.finalize();
    let encoded_hash = base16ct::lower::encode_string(&hash);
    if encoded_hash == expected_hash {
        return Ok(());
    } else {
        return Err(encoded_hash);
    }
}

#[starlark_module]
fn starlark_helpers(builder: &mut GlobalsBuilder) {
    fn download(url: &str, sha256_hash: &str) -> String {
        let checked_url = url::Url::parse(url).unwrap();
        let fname = checked_url.path_segments().unwrap().last().unwrap();
        if Path::new(fname).exists() {
            let contents = fs::read(fname)?;
            if check_hash_for_bytes(&contents, sha256_hash).is_ok() {
                println!("fname: {fname}");
                return Ok(fname.to_string());
            }
        }
        let body = reqwest::blocking::get(url).unwrap().bytes()?;
        if let Err(encoded_hash) = check_hash_for_bytes(&body, sha256_hash) {
            bail!("{encoded_hash} != {sha256_hash} for {url}")
        }

        fs::write(fname, body).unwrap();
        println!("fname: {fname}");
        Ok(fname.to_string())
    }

    fn unpack(fname: &str) -> String {
        let path = Path::new(fname);
        let mut absolute_path = std::env::current_dir()?;
        absolute_path.push(path);
        let pathed = path.file_stem().unwrap().to_str().unwrap();
        let info = eval.extra.unwrap().downcast_ref::<Info>().unwrap();
        let folder = Path::new(&info.hash_path).join(pathed);
        fs::create_dir_all(&folder)?;
        let current_dir = env::current_dir().context("can't get current dir!")?;
        env::set_current_dir(&folder).context(format!("can't set current dir to {:?}", folder))?;
        let output = Command::new("tar")
            .arg("-Jxvf")
            .arg(absolute_path.to_str().unwrap())
            .output()
            .context("from command")?;
        if output.status.code() != Some(0) {
            bail!(
                "Failed to run tar: {}, {}",
                std::str::from_utf8(&output.stdout)?,
                std::str::from_utf8(&output.stderr)?
            );
        }
        env::set_current_dir(current_dir)?;
        Ok(folder.to_str().unwrap().to_string())
    }

    fn cwd() -> String {
        Ok(std::env::current_dir()?.to_str().unwrap().to_string())
    }

    fn chdir(folder: &str) -> NoneType {
        let info = eval.extra.unwrap().downcast_ref::<Info>().unwrap();
        if !folder.starts_with(&info.hash_path) {
            bail!("{} is not a subfolder of {}", folder, info.hash_path);
        }
        env::set_current_dir(folder)?;
        println!("Cd to {}", folder);
        Ok(NoneType)
    }

    fn run(command: &str) -> i32 {
        let mut bits: VecDeque<_> = command.split(" ").collect();
        let program = bits.pop_front().unwrap();
        let output = Command::new(program)
            .args(bits)
            .output()
            .context(format!("from command for {}", program))?;
        if output.status.code() != Some(0) {
            bail!(
                "Failed to run {}: {}, {}",
                command,
                std::str::from_utf8(&output.stdout)?,
                std::str::from_utf8(&output.stderr)?
            );
        }
        Ok(0)
    }
}

pub fn build_module(filename: &str) -> Result<()> {
    // We first parse the content, giving a filename and the Starlark
    // `Dialect` we'd like to use (we pick standard).

    let content =
        fs::read_to_string(filename).context(format!("Error while loading '{filename}'"))?;
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let hash = hasher.finalize();

    let root_dir = Path::new("/home/palfrey/src/tuvix/");
    let store_dir = root_dir.join("store");
    let hash_dir = store_dir.join(base16ct::lower::encode_string(&hash));

    let ast: AstModule = AstModule::parse(filename, content.to_owned(), &Dialect::Extended)?;

    // We create a `Globals`, defining the standard library functions available.
    // The `standard` function uses those defined in the Starlark specification.
    let globals: Globals = GlobalsBuilder::new().with(starlark_helpers).build();

    // We create a `Module`, which stores the global variables for our calculation.
    let module: Module = Module::new();

    // We create an evaluator, which controls how evaluation occurs.
    let mut eval: Evaluator = Evaluator::new(&module);
    let info = Info {
        hash_path: hash_dir.to_str().unwrap().to_string(),
    };
    eval.extra = Some(&info);

    fs::create_dir_all(&hash_dir)?;
    std::env::set_current_dir(hash_dir)?;

    // And finally we evaluate the code using the evaluator.
    let build_fn = eval.eval_module(ast, &globals)?;
    let res = eval.eval_function(build_fn, &[], &[])?;
    println!("{:?}", res.unpack_str());
    Ok(())
}
