use anyhow::{Context, Result};
use starlark::environment::{Globals, Module};
use starlark::eval::Evaluator;
use starlark::syntax::{AstModule, Dialect};
use starlark::values::Value;
use std::fs;

pub fn build_module(filename: &str) -> Result<()> {
    // We first parse the content, giving a filename and the Starlark
    // `Dialect` we'd like to use (we pick standard).

    let content =
        fs::read_to_string(filename).context(format!("Error while loading '{filename}'"))?;

    let ast: AstModule = AstModule::parse(filename, content.to_owned(), &Dialect::Standard)?;

    // We create a `Globals`, defining the standard library functions available.
    // The `standard` function uses those defined in the Starlark specification.
    let globals: Globals = Globals::standard();

    // We create a `Module`, which stores the global variables for our calculation.
    let module: Module = Module::new();

    // We create an evaluator, which controls how evaluation occurs.
    let mut eval: Evaluator = Evaluator::new(&module);

    // And finally we evaluate the code using the evaluator.
    let res: Value = eval.eval_module(ast, &globals)?;
    assert_eq!(res.unpack_str(), Some("hello world!"));
    Ok(())
}
