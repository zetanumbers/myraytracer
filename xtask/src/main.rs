use std::{
    env,
    process::{Command, Stdio},
};

fn main() {
    let mut args = env::args();

    assert!(args.next().is_some());
    let subcommand = args.next();

    match subcommand.as_deref() {
        Some("build-wasm") => {
            let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());

            let mut command = Command::new(&cargo)
                .args(&[
                    "build",
                    "--message-format=json-render-diagnostics",
                    "--target=wasm32-unknown-unknown",
                    "--package=wasm-runner",
                ])
                .args(args)
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();

            let reader = std::io::BufReader::new(command.stdout.take().unwrap());

            let wasm_modules = cargo_metadata::Message::parse_stream(reader)
                .filter_map(|message| {
                    if let cargo_metadata::Message::CompilerArtifact(artifact) = message.unwrap() {
                        Some(artifact)
                    } else {
                        None
                    }
                })
                .filter(|artifact| {
                    artifact.target.name == "wasm-runner"
                        && artifact.target.kind.iter().any(|k| k == "cdylib")
                })
                .flat_map(|artifact| artifact.filenames)
                .filter(|filename| filename.extension() == Some("wasm"));

            let workspace_root = cargo_metadata::MetadataCommand::new()
                .no_deps()
                .other_options(["--offline"].map(|s| s.to_owned()))
                .exec()
                .unwrap()
                .workspace_root;
            let dump = workspace_root.join("pkg");

            let mut bindgen = wasm_bindgen_cli_support::Bindgen::new();

            for module in wasm_modules {
                bindgen.input_path(module);
            }

            let output = command.wait().expect("Couldn't get cargo's exit status");
            if !output.success() {
                std::process::exit(output.code().unwrap())
            }

            bindgen
                .web(true)
                .unwrap()
                .omit_default_module_path(false)
                .generate(&dump)
                .unwrap();
        }
        _ => print_help(),
    }
}

fn print_help() {
    eprintln!(
        "Tasks:
        build-wasm <cargo-build-args>       builds wasm module and runs wasm-bindgen
        "
    )
}
