use anyhow::{bail, Context, Result};
use reqwest::blocking::{Client, ClientBuilder};
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
use std::fs::{File, Permissions};
use std::os::unix::fs as unix_fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};
use tar::Archive;
use xz2::read::XzDecoder;

#[derive(AnyLifetime)]
struct Info {
    pub http_client: Client,
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
        let checked_url = url::Url::parse(url).expect(&format!("failure parsing '{}' as url", url));
        let fname = checked_url
            .path_segments()
            .expect("segments")
            .last()
            .expect("has last");
        if Path::new(fname).exists() {
            let contents = fs::read(fname)?;
            if check_hash_for_bytes(&contents, sha256_hash).is_ok() {
                return Ok(fname.to_string());
            }
        }
        let info = eval
            .extra
            .expect("has extra")
            .downcast_ref::<Info>()
            .expect("is info");
        let body = info.http_client.get(url).send()?.bytes()?;
        if let Err(encoded_hash) = check_hash_for_bytes(&body, sha256_hash) {
            bail!("{encoded_hash} != {sha256_hash} for {url}")
        }

        fs::write(fname, body).expect(&format!("can dump to {}", fname));
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
        println!("Running {}", command);
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
        println!(
            "Ran {}: '{}' '{}'",
            command,
            std::str::from_utf8(&output.stdout).unwrap(),
            std::str::from_utf8(&output.stderr).unwrap()
        );

        Ok(0)
    }

    fn get_output() -> String {
        Ok("/output".to_string())
    }

    fn joinpath(first: &str, second: &str) -> String {
        Ok(Path::new(first).join(second).to_str().unwrap().to_string())
    }

    fn r#move(source: &str, dest: &str) -> NoneType {
        fs::rename(source, dest).expect(&format!("Move {source} to {dest}"));
        Ok(NoneType)
    }

    fn mkdir(path: &str) -> NoneType {
        fs::create_dir(path).expect(&format!("Making directory {path}"));
        Ok(NoneType)
    }

    fn make_executable(path: &str) -> NoneType {
        fs::set_permissions(path, Permissions::from_mode(0o755))?;
        Ok(NoneType)
    }

    fn link(first: &str, second: &str) -> NoneType {
        fs::hard_link(first, second)?;
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

pub struct Builder {
    filename: String,
    content: String,
    hash_dir: PathBuf,
    module_dir: PathBuf,
}

fn make_if_not_exists(folder: &PathBuf) -> Result<()> {
    if !folder.exists() {
        fs::create_dir(&folder).context(format!("making {:?}", folder))?;
    }
    Ok(())
}

impl Builder {
    pub fn new(filename: &str) -> Result<Builder> {
        println!("Configuring {}", filename);
        let module_dir = Path::new(&filename)
            .parent()
            .unwrap()
            .canonicalize()
            .unwrap();

        let (content, hash) = hash_file(filename)?;

        let store_dir = root_dir().join("store");
        let hash_dir = store_dir.join(hash);

        Ok(Builder {
            filename: filename.to_string(),
            content,
            hash_dir,
            module_dir,
        })
    }

    pub fn build_in_chroot(self) -> Result<()> {
        let ast: AstModule =
            AstModule::parse(&self.filename, self.content.to_owned(), &Dialect::Extended)?;

        // We create a `Globals`, defining the standard library functions available.
        // The `standard` function uses those defined in the Starlark specification.
        let globals: Globals = GlobalsBuilder::extended().with(starlark_helpers).build();

        // We create a `Module`, which stores the global variables for our calculation.
        let module: Module = Module::new();

        // We create an evaluator, which controls how evaluation occurs.
        let mut eval: Evaluator = Evaluator::new(&module);
        let info = Info {
            http_client: ClientBuilder::new()
                .connection_verbose(true)
                .use_rustls_tls()
                .trust_dns(true)
                .build()?,
        };
        eval.extra = Some(&info);

        // And finally we evaluate the code using the evaluator.
        eval.eval_module(ast, &globals)?;

        let build_fn = module.get("build").context("Can't find build function")?;
        println!("Building {} in {:?}", self.filename, &self.hash_dir);

        let heap = eval.heap();

        let mut paths_map = SmallMap::new();
        paths_map.insert_hashed(heap.alloc_str("ncurses").get_hashed()?, heap.alloc("foo"));
        let paths = Dict::new(paths_map);
        let mut sb = StructBuilder::new(heap);
        sb.add("paths", paths);
        let build_context = sb.build().alloc_value(heap);

        // Make sure we do this *after* making the HTTP client, or that'll break due to needing various files
        // e.g. resolv.conf, NSS libs, etc
        unix_fs::chroot("./store/merged").context("can't chroot")?;
        env::set_current_dir("/")?;

        make_if_not_exists(&PathBuf::from("/output"))?;
        let res = eval.eval_function(build_fn, &[build_context], &[])?;
        if res.is_none() {
            println!("Build complete for {}", self.filename);
        } else {
            println!("Build result for {}: {:?}", self.filename, res.unpack_str());
        }
        Ok(())
    }

    pub fn build_module(&self) -> Result<()> {
        let complete_file = self.hash_dir.join(".complete");

        if complete_file.exists() {
            println!("{} is already built in {:?}", self.filename, &self.hash_dir);
            return Ok(());
        }

        let ast: AstModule =
            AstModule::parse(&self.filename, self.content.to_owned(), &Dialect::Extended)?;

        // We create a `Globals`, defining the standard library functions available.
        // The `standard` function uses those defined in the Starlark specification.
        let globals: Globals = GlobalsBuilder::extended().with(starlark_helpers).build();

        // We create a `Module`, which stores the global variables for our calculation.
        let module: Module = Module::new();

        // We create an evaluator, which controls how evaluation occurs.
        let mut eval: Evaluator = Evaluator::new(&module);
        let info = Info {
            http_client: Client::new(),
        };
        eval.extra = Some(&info);

        // And finally we evaluate the code using the evaluator.
        eval.eval_module(ast, &globals)?;

        fs::create_dir_all(&self.hash_dir)?;

        let heap = eval.heap();

        let dependencies = module
            .get("dependencies")
            .unwrap_or_else(|| heap.alloc_list(&[]));

        let mut dep_outputs = vec![];
        for dep in dependencies.iterate(heap).unwrap() {
            let dep_path = self
                .module_dir
                .join(format!("{}.star", dep.unpack_str().unwrap()));
            println!("Loading {:?}", dep_path);
            let builder = Builder::new(dep_path.to_str().unwrap())?;
            builder.build_module()?;
            dep_outputs.push(
                builder
                    .hash_dir
                    .join("output")
                    .canonicalize()?
                    .to_str()
                    .unwrap()
                    .to_string(),
            );
        }

        let mut mount_args = vec![
            String::from("python"),
            String::from("./helpers/mount-all.py"),
            String::from("./store"),
            self.hash_dir
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
        ];
        mount_args.append(&mut dep_outputs);
        println!("Mounting: {}", mount_args.join(" "));
        let mount_output = Command::new("sudo")
            .args(&mount_args)
            .output()
            .expect("mounting");

        if mount_output.status.code() != Some(0) {
            bail!(
                "Failed to run {:?}: '{}' '{}'",
                mount_args,
                std::str::from_utf8(&mount_output.stdout).unwrap(),
                std::str::from_utf8(&mount_output.stderr).unwrap()
            );
        }

        let chroot_builder_path = "target/debug/build_in_chroot";

        let output = Command::new("sudo")
            //.arg("strace")
            .arg("-E")
            .arg(chroot_builder_path)
            .arg(&self.filename)
            .env_clear()
            //.env("RUST_LOG", "trace")
            .output()
            .expect("launch build_in_chroot");

        let unmount_output = Command::new("sudo")
            .args(vec!["python", "./helpers/unmount-all.py", "./store"])
            .output()
            .expect("unmounting");

        if unmount_output.status.code() != Some(0) {
            bail!(
                "Failed to run unmount: '{}' '{}'",
                std::str::from_utf8(&unmount_output.stdout).unwrap(),
                std::str::from_utf8(&unmount_output.stderr).unwrap()
            );
        }

        if output.status.code() != Some(0) {
            bail!(
                "Failed to run {}: '{}' '{}'",
                chroot_builder_path,
                std::str::from_utf8(&output.stdout).unwrap(),
                std::str::from_utf8(&output.stderr).unwrap()
            );
        }
        println!(
            "Ran {}: '{}' '{}'",
            self.filename,
            std::str::from_utf8(&output.stdout).unwrap(),
            std::str::from_utf8(&output.stderr).unwrap()
        );
        let complete_file = self.hash_dir.join(".complete");
        fs::write(&complete_file, "")
            .context(format!("issues while writing {:?}", &complete_file))?;

        Ok(())
    }
}
