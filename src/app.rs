use gtk::{gio, glib};
use gtk::prelude::*;
use adw::subclass::prelude::*;

use crate::window::PacViewWindow;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct PacViewApplication {}

    #[glib::object_subclass]
    impl ObjectSubclass for PacViewApplication {
        const NAME: &'static str = "PacViewApplication";
        type Type = super::PacViewApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for PacViewApplication {}

    impl ApplicationImpl for PacViewApplication {
        fn activate(&self) {
            let application = self.obj();

            let window = if let Some(window) = application.active_window() {
                window
            } else {
                let window = PacViewWindow::new(&*application);
                window.upcast()
            };

            application.set_accels_for_action("win.show-sidebar", &["<ctrl>b"]);
            application.set_accels_for_action("search.search-start", &["<ctrl>f"]);
            application.set_accels_for_action("search.search-stop", &["Escape"]);

            window.present();
        }
    }

    impl GtkApplicationImpl for PacViewApplication {}
    impl AdwApplicationImpl for PacViewApplication {}
}

glib::wrapper! {
    pub struct PacViewApplication(ObjectSubclass<imp::PacViewApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl PacViewApplication {
    pub fn new(application_id: &str, flags: &gio::ApplicationFlags) -> Self {
        glib::Object::builder()
            .property("application-id", application_id)
            .property("flags", flags)
            .build()
    }
}
