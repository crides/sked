pub fn filename_to_module(filename: &str) -> String {
    use std::path::Path;
    let filename = filename.trim_end_matches('/');
    let path = Path::new(filename);
    let name = path.extension().map_or(filename, |ext| {
        ext.to_str()
            .and_then(|ext| {
                if ext == "glu" {
                    Some(&filename[..filename.len() - ext.len() - 1])
                } else {
                    None
                }
            })
            .unwrap_or(filename)
    });

    name.trim_start_matches(|c: char| c == '.' || c == '/')
        .replace(|c: char| c == '/' || c == '\\', ".")
}

fn generate_std_include() {
    let tuples = WalkDir::new("gluon")
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.file_type().is_file()
                && entry.path().extension() == Some(::std::ffi::OsStr::new("glu"))
        })
        .map(|entry| {
            let module_name = filename_to_module(entry.path().to_str().expect("Invalid path"));
            format!(
                r#"("{}", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/{}"))),"#,
                module_name,
                entry.path().display().to_string().replace('\\', "/")
            )
        })
        .format("\n");

    let out_file_name = Path::new(&env::var("OUT_DIR").unwrap()).join("sched_modules.rs");
    let mut file = File::create(&out_file_name).unwrap();

    write!(
        file,
        r#"static SCHED_LIBS: &[(&str, &str)] = "#
    )
    .unwrap();
    writeln!(file, "&[{}];", tuples).unwrap();
}

fn main() {
    generate_std_include();
    println!("cargo:rerun-if-changed=std/");
}
