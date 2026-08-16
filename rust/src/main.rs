fn main() {
    if let Err(error) = kadishutu::cli::run() {
        eprintln!("{error}");
        std::process::exit(error.exit_code());
    }
}
