#!/usr/bin/python

from sys import argv
from pathlib import Path
from subprocess import check_call, check_output

assert len(argv) == 2, argv

def run(cmd):
    print(cmd)
    check_call(cmd.split(" "))

store_folder = Path(argv[1]).resolve(strict=True)
merged_dir = store_folder.joinpath("merged")

proc_dir = merged_dir.joinpath("proc")

if proc_dir.joinpath("version").exists():
    run(f"umount {proc_dir}")

dev_dir = merged_dir.joinpath("dev")

if dev_dir.joinpath("null").exists():
    run(f"umount {dev_dir}")

mounts = check_output("mount", encoding="utf-8")

if merged_dir.as_posix() in mounts:
    run(f"umount {merged_dir}")
