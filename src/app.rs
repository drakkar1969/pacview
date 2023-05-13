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

    impl ObjectImpl for PacViewApplication {
        fn constructed(&self) {
            self.parent_constructed();

            self.obj().setup_actions();
        }
    }

    impl ApplicationImpl for PacViewApplication {
        fn activate(&self) {
            let application = self.obj();

            let window = if let Some(window) = application.active_window() {
                window
            } else {
                let window = PacViewWindow::new(&*application);
                window.upcast()
            };

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

    fn setup_actions(&self) {
        let quit_action = gio::ActionEntry::builder("quit-app")
            .activate(move |app: &Self, _, _| app.quit())
            .build();
        let about_action = gio::ActionEntry::builder("show-about")
            .activate(move |app: &Self, _, _| app.show_about())
            .build();
        self.add_action_entries([quit_action, about_action]);

        self.set_accels_for_action("app.quit-app", &["<ctrl>q"]);
        self.set_accels_for_action("app.show-about", &["F1"]);

        self.set_accels_for_action("win.show-sidebar", &["<ctrl>b"]);
        self.set_accels_for_action("win.show-infopane", &["<ctrl>i"]);

        self.set_accels_for_action("search.start", &["<ctrl>f"]);
        self.set_accels_for_action("search.stop", &["Escape"]);

        self.set_accels_for_action("view.refresh", &["F5"]);
    }

    fn show_about(&self) {
        let window = self.active_window().unwrap();

        let about_window = adw::AboutWindow::builder()
            .transient_for(&window)
            .application_name("PacView")
            .application_icon("software-properties")
            .developer_name("draKKar1969")
            .version("1.0.0")
            .website("https://github.com/drakkar1969/pacview")
            .developers(vec!["draKKar1969"])
            .designers(vec!["draKKar1969"])
            .copyright("© 2023 draKKar1969")
            .license_type(gtk::License::Gpl30)
            .build();

        about_window.present();
    }
}
