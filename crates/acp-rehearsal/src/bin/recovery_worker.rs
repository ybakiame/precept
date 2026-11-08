fn main() {
    if std::env::args().skip(1).collect::<Vec<_>>() != ["--synthetic-only"] {
        eprintln!("SyntheticOnlyRequired");
        std::process::exit(2);
    }
    acp_rehearsal::recovery::worker_main();
}
