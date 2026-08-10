// Na Windows nechceme za oknem viset konzoli.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    textura_app_lib::run()
}
