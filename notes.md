nightly-2022-02-26
fakechroot - breaks stuff https://github.com/dex4er/fakechroot/issues/48

https://github.com/robxu9/bash-static/releases/download/5.1.016-1.2.2/bash-linux-x86_64
https://github.com/yunchih/static-binaries/blob/master/strace?raw=true
https://github.com/yunchih/static-binaries/blob/master/wget?raw=true

https://jvns.ca/blog/2019/11/18/how-containers-work--overlayfs/

SUDO_ASKPASS=/bin/ssh-askpass cargo watch --watch src -s "cargo fmt && cargo build --tests && cargo test -- --nocapture"