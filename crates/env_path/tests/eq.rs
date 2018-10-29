extern crate env_path;

use env_path::EnvPath;
use std::env;
use std::path::PathBuf;

#[test]
fn test_path_comparison() {
    let home = env::var("HOME").unwrap();
    let env_path = EnvPath::new("$HOME");
    let path = PathBuf::from(home);

    assert_eq!(env_path, path);
}

#[test]
fn test_addr_comparison() {
    let env_path = EnvPath::new("$HOME");
    let string = String::from("$HOME");
    let literal = "$HOME";

    assert_eq!(env_path, string);
    assert_eq!(env_path, literal);
}
