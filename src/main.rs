mod app;
mod window;
mod search_header;
mod search_tag;
mod filter_row;
mod package_view;
mod info_pane;
mod text_widget;
mod property_label;
mod property_value;
mod preferences_dialog;
mod stats_dialog;
mod backup_dialog;
mod log_dialog;
mod details_dialog;
mod pkg_object;
mod stats_object;
mod backup_object;
mod log_object;
mod utils;

use gtk::{gio, glib};
use gtk::prelude::*;

use app::PacViewApplication;

const APP_ID: &str = "com.github.PacView";

fn main() -> glib::ExitCode {
    // Register and include resources
    gio::resources_register_include!("resources.gresource")
        .expect("Failed to register resources");

    // Run app
    let app = PacViewApplication::new(APP_ID, &gio::ApplicationFlags::empty());

    app.run()
}
