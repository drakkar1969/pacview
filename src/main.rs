mod app;
mod window;
mod search_header;
mod search_tag;
mod filter_row;
mod value_row;
mod info_pane;
mod preferences_window;
mod stats_window;
mod toggle_button;
mod details_window;
mod pkg_object;
mod prop_object;
mod stats_object;
mod backup_object;
mod utils;

use gtk::{gio, glib};
use gtk::prelude::*;

use app::PacViewApplication;

const APP_ID: &str = "com.github.PacView";

fn main() -> glib::ExitCode {
    // Register and include resources
    gio::resources_register_include!("resources.gresource")
        .expect("Failed to register resources.");

    // Run app
    let app = PacViewApplication::new(APP_ID, &gio::ApplicationFlags::empty());

    app.run()
}
