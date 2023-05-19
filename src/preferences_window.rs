use std::cell::RefCell;

use gtk::glib;
use gtk::subclass::prelude::*;
use adw::subclass::prelude::*;
use gtk::prelude::*;

// use crate::pkg_object::PkgFlags;

//------------------------------------------------------------------------------
// MODULE: PREFERENCESWINDOW
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PreferencesWindow)]
    #[template(resource = "/com/github/PacView/ui/preferences_window.ui")]
    pub struct PreferencesWindow {
        // #[template_child]
        // pub stack: TemplateChild<gtk::Stack>,

        #[property(get, set)]
        aur_command: RefCell<String>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesWindow {
        const NAME: &'static str = "PreferencesWindow";
        type Type = super::PreferencesWindow;
        type ParentType = adw::PreferencesWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PreferencesWindow {
        //-----------------------------------
        // Default property functions
        //-----------------------------------
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();
        }
    }

    impl WidgetImpl for PreferencesWindow {}
    impl WindowImpl for PreferencesWindow {}
    impl AdwWindowImpl for PreferencesWindow {} 
    impl PreferencesWindowImpl for PreferencesWindow {}
}

//------------------------------------------------------------------------------
// PUBLIC IMPLEMENTATION
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PreferencesWindow(ObjectSubclass<imp::PreferencesWindow>)
        @extends adw::Window, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl PreferencesWindow {
    pub fn new() -> Self {
        glib::Object::builder()
            // .property("icon", icon)
            // .property("text", text)
            .build()
    }
}

// impl Default for FilterRow {
//     fn default() -> Self {
//         Self::new("", "")
//     }
// }
