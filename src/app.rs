use gtk::{gio, glib};
use gtk::prelude::*;
use adw::subclass::prelude::*;
use adw::prelude::AdwDialogExt;

use crate::window::PacViewWindow;

//------------------------------------------------------------------------------
// MODULE: PacViewApplication
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default)]
    pub struct PacViewApplication {}

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PacViewApplication {
        const NAME: &'static str = "PacViewApplication";
        type Type = super::PacViewApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for PacViewApplication {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            self.obj().setup_actions();
        }
    }

    impl ApplicationImpl for PacViewApplication {
        //---------------------------------------
        // Activate handler
        //---------------------------------------
        fn activate(&self) {
            let application = self.obj();

            // Show main window
            let window = if let Some(window) = application.active_window() {
                window
            } else {
                let window = PacViewWindow::new(&application);
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
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(application_id: &str, flags: gio::ApplicationFlags) -> Self {
        glib::Object::builder()
            .property("application-id", application_id)
            .property("flags", flags)
            .build()
    }

    //---------------------------------------
    // Setup actions
    //---------------------------------------
    fn setup_actions(&self) {
        // Add quit action
        let quit_action = gio::ActionEntry::builder("quit-app")
            .activate(move |app: &Self, _, _| app.quit())
            .build();

        // Add show about dialog action
        let about_action = gio::ActionEntry::builder("show-about")
            .activate(move |app: &Self, _, _| {
                let window = app.active_window()
                    .expect("Could not retrieve active window");

                let about_dialog = adw::AboutDialog::builder()
                    .application_name("PacView")
                    .application_icon("software-properties")
                    .developer_name("draKKar1969")
                    .version(env!("CARGO_PKG_VERSION"))
                    .website("https://github.com/drakkar1969/pacview")
                    .developers(["draKKar1969"])
                    .designers(["draKKar1969"])
                    .copyright("© 2023 draKKar1969")
                    .license_type(gtk::License::Gpl30)
                    .build();

                about_dialog.present(Some(&window));
            })
            .build();

        // Add actions to app
        self.add_action_entries([quit_action, about_action]);

        // Add app keyboard shortcuts
        self.set_accels_for_action("app.quit-app", &["<ctrl>Q"]);
    }
}
