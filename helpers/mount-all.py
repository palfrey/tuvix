#!/usr/bin/python

from sys import argv
from pathlib import Path
from subprocess import check_call, check_output

assert len(argv) >= 2, argv

def run(cmd):
    print(cmd)
    check_call(cmd.split(" "))

store_folder = Path(argv[1]).resolve(strict=True)
store_hash = argv[2]
others = [Path(x).resolve(strict=True) for x in argv[3:]]

upper_dir = store_folder.joinpath(store_hash)

working_dir = store_folder.joinpath("working")
working_dir.mkdir(exist_ok=True)

special_dir = store_folder.joinpath("special")
special_dir.mkdir(exist_ok=True)

proc_dir = special_dir.joinpath("proc")
proc_dir.mkdir(exist_ok=True)

dev_dir = special_dir.joinpath("dev")
dev_dir.mkdir(exist_ok=True)

tmp_dir = special_dir.joinpath("tmp")
tmp_dir.mkdir(exist_ok=True)

merged_dir = store_folder.joinpath("merged")
merged_dir.mkdir(exist_ok=True)

lowers = others + [special_dir]

mounts = check_output("mount", encoding="utf-8")

if merged_dir.as_posix() not in mounts:
    cmd = f"mount -t overlay overlay -o lowerdir={':'.join([str(x) for x in lowers])},upperdir={upper_dir},workdir={working_dir} {merged_dir}"
    run(cmd)

proc_dir = merged_dir.joinpath("proc")

if not proc_dir.joinpath("version").exists():
    cmd = f"mount --bind /proc {proc_dir}"
    run(cmd)

dev_dir = merged_dir.joinpath("dev")

if not dev_dir.joinpath("null").exists():
    cmd = f"mount --bind /dev {dev_dir}"
    run(cmd)
