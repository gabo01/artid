#[macro_export]
macro_rules! tmpdir {
    () => {
        tempfile::tempdir().expect("Unable to create tmp directory");
    };
}

#[macro_export]
macro_rules! tmppath {
    ($dir:expr, $path:expr) => {
        $dir.path().join($path)
    };
}

#[macro_export]
macro_rules! create_file {
    ($path:expr) => {
        {
            let _file = File::create($path).expect("Unable to create file");
            $path
        }
    };

    ($path:expr, $($arg:tt)*) => {
        {
            use std::io::Write;

            let mut file = File::create($path).expect("Unable to create file");
            write!(file, $($arg)*).expect("Unable to write to file");
            $path
        }
    }
}

#[macro_export]
macro_rules! read_file {
    ($file:expr) => {{
        use std::io::Read;

        let mut file = File::open($file).expect("Unable to open file");
        let mut buf = String::new();
        file.read_to_string(&mut buf).expect("Unable to read file");
        buf
    }};
}

#[macro_export]
macro_rules! symlink {
    ($path:expr) => {
        filetype!($path).is_symlink()
    };
}

#[macro_export]
macro_rules! filetype {
    ($path:expr) => {{
        use std::fs;
        fs::symlink_metadata($path)
            .expect("Unable to get metadata")
            .file_type()
    }};
}
