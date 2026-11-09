use acp_ui_spike_model::native::Inputs;
fn main() {
    match Inputs::parse().fetch() {
        Ok(snapshot) => println!("{}", serde_json::to_string(&snapshot).unwrap()),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    }
}
