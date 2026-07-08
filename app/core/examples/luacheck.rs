//! Syntax-check a Lua file without executing it.
fn main() {
    let path = std::env::args().nth(1).expect("usage: luacheck <file.lua>");
    let src = std::fs::read_to_string(&path).unwrap();
    let lua = mlua::Lua::new();
    match lua.load(&src).into_function() {
        Ok(_) => println!("syntax OK: {path}"),
        Err(e) => { eprintln!("SYNTAX ERROR: {e}"); std::process::exit(1); }
    }
}
