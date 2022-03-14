use std::path::Path;

use tuvix::Builder;

#[test]
fn build_zsh() {
    let test_path = Path::new(file!());
    Builder::new(
        test_path
            .parent()
            .unwrap()
            .join("zsh.star")
            .to_str()
            .unwrap(),
    )
    .unwrap()
    .build_module()
    .unwrap();
}
