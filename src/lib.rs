use anyhow::{anyhow, bail, Context, Result};
use libmount::BindMount;
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
use starlark::collections::SmallMap;
use starlark::environment::{Globals, GlobalsBuilder, Module};
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::syntax::{AstModule, Dialect};
use starlark::values::dict::Dict;
use starlark::values::none::NoneType;
use starlark::values::structs::StructBuilder;
use starlark::values::{AllocValue, AnyLifetime};
use std::collections::VecDeque;
use std::fs::File;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};
use tar::Archive;
use xz2::read::XzDecoder;

#[derive(AnyLifetime)]
struct Info {
    pub http_client: Client,
    pub hash_dir: PathBuf,
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
                return Ok(fname.to_string());
            }
        }
        let info = eval.extra.unwrap().downcast_ref::<Info>().unwrap();
        let body = info.http_client.get(url).send()?.bytes()?;
        if let Err(encoded_hash) = check_hash_for_bytes(&body, sha256_hash) {
            bail!("{encoded_hash} != {sha256_hash} for {url}")
        }

        fs::write(fname, body).unwrap();
        Ok(fname.to_string())
    }

    fn unpack(fname: &str) -> String {
        let path = Path::new(fname);
        let mut absolute_path = std::env::current_dir()?;
        absolute_path.push(path);
        let pathed = path.file_stem().unwrap().to_str().unwrap();
        let folder = Path::new("/").join(&pathed);
        fs::create_dir_all(&folder)?;
        let compressed_file = File::open(path)?;
        let decompressor = XzDecoder::new(compressed_file);
        let mut archive = Archive::new(decompressor);
        archive.unpack(&folder)?;
        Ok(folder.to_str().unwrap().to_string())
    }

    fn cwd() -> String {
        Ok(std::env::current_dir()?.to_str().unwrap().to_string())
    }

    fn chdir(folder: &str) -> NoneType {
        println!("CD to {folder}");
        env::set_current_dir(folder)?;
        Ok(NoneType)
    }

    fn run(command: &str) -> i32 {
        let mut bits: VecDeque<_> = command.split(" ").collect();
        let program = bits.pop_front().expect("pop program");
        let hash_dir = eval
            .extra
            .unwrap()
            .downcast_ref::<Info>()
            .unwrap()
            .hash_dir
            .to_str()
            .unwrap()
            .as_bytes()
            .to_owned();
        println!("hash_dir: {}", std::str::from_utf8(&hash_dir).unwrap());
        let output = Command::new(program)
            .args(&bits)
            .env_clear()
            .output()
            .expect("spawn");
        if output.status.code() != Some(0) {
            bail!(
                "Failed to run {}: '{}' '{}'",
                command,
                std::str::from_utf8(&output.stdout).unwrap(),
                std::str::from_utf8(&output.stderr).unwrap()
            );
        }

        Ok(0)
    }

    fn get_output() -> String {
        Ok(eval
            .extra
            .unwrap()
            .downcast_ref::<Info>()
            .unwrap()
            .hash_dir
            .join("output")
            .to_str()
            .unwrap()
            .to_string())
    }

    fn joinpath(first: &str, second: &str) -> String {
        Ok(Path::new(first).join(second).to_str().unwrap().to_string())
    }

    fn r#move(source: &str, dest: &str) -> NoneType {
        Ok(NoneType)
    }
}

fn hash_file(filename: &str) -> Result<(String, String)> {
    let content =
        fs::read_to_string(filename).context(format!("Error while loading '{filename}'"))?;
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let hash = hasher.finalize();
    Ok((content, base16ct::lower::encode_string(&hash)))
}

fn root_dir() -> PathBuf {
    PathBuf::from("/home/palfrey/src/tuvix/")
}

fn make_if_not_exists(folder: &PathBuf) -> Result<()> {
    if !folder.exists() {
        fs::create_dir(&folder).context(format!("making {:?}", folder))?;
    }
    Ok(())
}

fn setup_hashdir(hash_dir: &PathBuf) -> Result<()> {
    let bin_path = hash_dir.join("bin");
    make_if_not_exists(&bin_path)?;

    let hidden_bin_path = hash_dir.join(".bin");
    make_if_not_exists(&hidden_bin_path)?;
    fs::copy(
        root_dir().join("helpers/strace"),
        hidden_bin_path.join("strace"),
    )
    .context("copying strace")?;

    let dev_path = hash_dir.join("dev");
    make_if_not_exists(&dev_path)?;
    if !dev_path.join("null").exists() {
        let dev_mount = BindMount::new("/dev", dev_path);
        dev_mount.mount().map_err(|err| anyhow!("{}", err))?;
    }
    let proc_path = hash_dir.join("proc");
    make_if_not_exists(&proc_path)?;
    if !proc_path.join("version").exists() {
        let proc_mount = BindMount::new("/proc", proc_path);
        proc_mount.mount().map_err(|err| anyhow!("{}", err))?;
    }

    let tmp_path = hash_dir.join("tmp");
    make_if_not_exists(&tmp_path)?;

    Ok(())
}

pub fn build_module(filename: &str) -> Result<()> {
    println!("Configuring {}", filename);
    let module_dir = Path::new(&filename)
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap();

    let (content, hash) = hash_file(filename)?;

    let store_dir = root_dir().join("store");
    let hash_dir = store_dir.join(hash);
    let complete_file = hash_dir.join(".complete");

    if complete_file.exists() {
        println!("{} is already built in {:?}", filename, &hash_dir);
        return Ok(());
    }

    let ast: AstModule = AstModule::parse(filename, content.to_owned(), &Dialect::Extended)?;

    // We create a `Globals`, defining the standard library functions available.
    // The `standard` function uses those defined in the Starlark specification.
    let globals: Globals = GlobalsBuilder::extended().with(starlark_helpers).build();

    // We create a `Module`, which stores the global variables for our calculation.
    let module: Module = Module::new();

    // We create an evaluator, which controls how evaluation occurs.
    let mut eval: Evaluator = Evaluator::new(&module);
    let info = Info {
        http_client: Client::new(),
        hash_dir: hash_dir.clone(),
    };
    eval.extra = Some(&info);

    fs::create_dir_all(&hash_dir)?;
    std::env::set_current_dir(&hash_dir)?;

    // And finally we evaluate the code using the evaluator.
    eval.eval_module(ast, &globals)?;

    let heap = eval.heap();

    let dependencies = module
        .get("dependencies")
        .unwrap_or_else(|| heap.alloc_list(&[]));

    for dep in dependencies.iterate(heap).unwrap() {
        let dep_path = module_dir.join(format!("{}.star", dep.unpack_str().unwrap()));
        println!("Loading {:?}", dep_path);
        build_module(dep_path.to_str().unwrap()).unwrap();
    }

    let build_fn = module.get("build").context("Can't find build function")?;
    println!("Building {} in {:?}", filename, &hash_dir);

    let mut paths_map = SmallMap::new();
    paths_map.insert_hashed(heap.alloc_str("ncurses").get_hashed()?, heap.alloc("foo"));
    let paths = Dict::new(paths_map);
    let mut sb = StructBuilder::new(heap);
    sb.add("paths", paths);
    let build_context = sb.build().alloc_value(heap);

    setup_hashdir(&hash_dir)?;
    unix_fs::chroot(&hash_dir).context("can't chroot")?;

    let res = eval.eval_function(build_fn, &[build_context], &[])?;
    if res.is_none() {
        println!("Build complete for {}", filename);
    } else {
        println!("Build result for {}: {:?}", filename, res.unpack_str());
    }
    fs::write(&complete_file, "").context(format!("issues while writing {:?}", &complete_file))?;

    Ok(())
}
