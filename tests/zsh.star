def build(context):
    path = download("https://www.zsh.org/pub/zsh-5.8.1.tar.xz", "b6973520bace600b4779200269b1e5d79e5f505ac4952058c11ad5bbf0dd9919")
    destination = unpack(path)
    current_folder = cwd()
    chdir(destination + "/zsh-5.8.1")
    run("./configure --prefix=%s --with-tcsetpgrp" % current_folder)
    run("make -j")
    run("make install")
