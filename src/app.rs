use gtk::{gio, glib};
use gtk::prelude::*;
use adw::subclass::prelude::*;

use crate::window::PacViewWindow;

//------------------------------------------------------------------------------
// MODULE: PacViewApplication
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default)]
    pub struct PacViewApplication {}

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PacViewApplication {
        const NAME: &'static str = "PacViewApplication";
        type Type = super::PacViewApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for PacViewApplication {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            self.obj().setup_actions();
        }
    }

    impl ApplicationImpl for PacViewApplication {
        //-----------------------------------
        // Activate handler
        //-----------------------------------
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

//------------------------------------------------------------------------------
// IMPLEMENTATION: PacViewApplication
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PacViewApplication(ObjectSubclass<imp::PacViewApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl PacViewApplication {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(application_id: &str, flags: &gio::ApplicationFlags) -> Self {
        glib::Object::builder()
            .property("application-id", application_id)
            .property("flags", flags)
            .build()
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        let quit_action = gio::ActionEntry::builder("quit-app")
            .activate(move |app: &Self, _, _| app.quit())
            .build();
        let about_action = gio::ActionEntry::builder("show-about")
            .activate(move |app: &Self, _, _| app.show_about())
            .build();
        self.add_action_entries([quit_action, about_action]);

        self.set_accels_for_action("app.quit-app", &["<ctrl>Q"]);
        self.set_accels_for_action("app.show-about", &["F1"]);

        self.set_accels_for_action("win.show-sidebar", &["<ctrl>B"]);
        self.set_accels_for_action("win.show-infopane", &["<ctrl>I"]);
        self.set_accels_for_action("win.show-preferences", &["<ctrl>comma"]);

        self.set_accels_for_action("search.toggle", &["<ctrl>F"]);
        self.set_accels_for_action("search.stop", &["Escape"]);

        self.set_accels_for_action("view.refresh", &["F5"]);
        self.set_accels_for_action("view.show-stats", &["<alt>S"]);
        self.set_accels_for_action("view.copy-list", &["<alt>L"]);

        self.set_accels_for_action("info.previous", &["<alt>Left"]);
        self.set_accels_for_action("info.next", &["<alt>Right"]);
        self.set_accels_for_action("info.show-details", &["<alt>Return", "<alt>KP_Enter"]);
    }

    //-----------------------------------
    // Show about window
    //-----------------------------------
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
