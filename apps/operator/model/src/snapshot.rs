use acp_operator_model::native::Inputs;
fn main() {
    match Inputs::parse().fetch() {
        Ok(snapshot) => println!("{}", serde_json::to_string(&snapshot).unwrap()),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(if message == "Permission denied" {
                acp_operator_model::native::HELPER_ACCESS_REFUSED
            } else {
                2
            });
        }
    }
}
