use gtk::{gio, glib, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;

use crate::pkg_object::PkgObject;

//------------------------------------------------------------------------------
// MODULE: DetailsWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/details_window.ui")]
    pub struct DetailsWindow {
        #[template_child]
        pub pkg_label: TemplateChild<gtk::Label>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for DetailsWindow {
        const NAME: &'static str = "DetailsWindow";
        type Type = super::DetailsWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for DetailsWindow {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();
        }
    }

    impl WidgetImpl for DetailsWindow {}
    impl WindowImpl for DetailsWindow {}
    impl ApplicationWindowImpl for DetailsWindow {}
    impl AdwApplicationWindowImpl for DetailsWindow {}

    #[gtk::template_callbacks]
    impl DetailsWindow {
        #[template_callback]
        fn on_key_pressed(&self, key: u32, _: u32, state: gdk::ModifierType) -> bool {
            if key == 65307 && state.is_empty() {
                self.obj().close();
            }

            true
        }
    }
}

//------------------------------------------------------------------------------
// PUBLIC IMPLEMENTATION: DetailsWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct DetailsWindow(ObjectSubclass<imp::DetailsWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl DetailsWindow {
    //-----------------------------------
    // Public new function
    //-----------------------------------
    pub fn new(pkg: Option<PkgObject>) -> Self {
        let win: Self = glib::Object::builder().build();

        if let Some(pkg) = pkg {
            win.setup_banner(pkg);
        }

        win
    }

    fn setup_banner(&self, pkg: PkgObject) {
        self.imp().pkg_label.set_label(&format!("{repo}/{name}", repo=pkg.repo_show(), name=pkg.name()));
    }
}
