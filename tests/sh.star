def build(context):
    path = download(
        "https://github.com/robxu9/bash-static/releases/download/5.1.016-1.2.2/bash-linux-x86_64",
        "855095c198be4505f1b16e95f4740ac30bcfff888961b4692146fd518bda12cd"
    )
    output_folder = get_output()
    bin_folder = joinpath(output_folder, "bin")
    mkdir(bin_folder)
    sh_path = joinpath(bin_folder, "sh")
    move(path, sh_path)