extern crate env_path;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use env_path::EnvPath;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Test {
    path: EnvPath,
}

#[test]
fn serialize() {
    let path = EnvPath::new("/home/gabo01");
    assert_eq!("\"/home/gabo01\"", serde_json::to_string(&path).unwrap());
}

#[test]
fn deserialize() {
    let data = r#"{
        "path": "/home/gabo01"
    }"#;

    match serde_json::from_str::<Test>(data) {
        Ok(test) => {
            assert_eq!(
                test,
                Test {
                    path: EnvPath::new("/home/gabo01")
                }
            );
        }

        Err(_) => {
            panic!("Error in deserialization");
        }
    };
}
