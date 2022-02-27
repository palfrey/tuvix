def build(context):
    path = download("https://www.zsh.org/pub/zsh-5.8.1.tar.xz", "abc")
    destination = unpack(path)
    