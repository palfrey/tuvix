def build(context):
    path = download(
        "https://github.com/palfrey/tuvix/blob/main/helpers/sed?raw=true",
        "875819cf816844a12fd0c25725cd3ea3f5d91fd0c75064c23c4791755e51fabe"
    )
    output_folder = get_output()
    bin_folder = joinpath(output_folder, "bin")
    mkdir(bin_folder)
    sed_path = joinpath(bin_folder, "sed")
    move(path, sed_path)
    make_executable(sed_path)