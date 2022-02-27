use std::path::Path;

use tuvix::build_module;

#[test]
fn build_zsh() {
    let test_path = Path::new(file!());
    build_module(
        test_path
            .parent()
            .unwrap()
            .join("zsh.star")
            .to_str()
            .unwrap(),
    )
    .unwrap();
}
