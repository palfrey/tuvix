dependencies = ["sh", "sed", "busybox"]

def build(context):
    path = download("https://www.zsh.org/pub/zsh-5.8.1.tar.xz", "b6973520bace600b4779200269b1e5d79e5f505ac4952058c11ad5bbf0dd9919")
    destination = unpack(path)
    chdir(destination + "/zsh-5.8.1")
    run("./configure --prefix=/usr --with-tcsetpgrp --build=x86_64-unknown-linux-gnu")
    run("make -j")
    run("make install")
