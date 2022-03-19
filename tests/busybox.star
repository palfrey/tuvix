def build(context):
    path = download(
        "https://github.com/ryanwoodsmall/static-binaries/blob/master/x86_64/busybox?raw=true",
        "9b310702887098419191a367072528e06b7c8350e2228628a5e761aeda42f8e4"
    )
    output_folder = get_output()
    bin_folder = joinpath(output_folder, "bin")
    mkdir(bin_folder)
    busybox_path = joinpath(bin_folder, "busybox")
    move(path, busybox_path)
    make_executable(busybox_path)
    for executable in ["rm", "chmod", "ls", "expr", "cat", "sort"]:
        link(busybox_path, joinpath(bin_folder, executable))