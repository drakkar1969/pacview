use std::cell::{Cell, RefCell};

use gtk::glib;
use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::clone;

//------------------------------------------------------------------------------
// MODULE: PreferencesWindow
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
        #[template_child]
        pub aur_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub column_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub sort_switch: TemplateChild<gtk::Switch>,

        #[property(get, set)]
        aur_command: RefCell<String>,
        #[property(get, set)]
        remember_columns: Cell<bool>,
        #[property(get, set)]
        remember_sort: Cell<bool>,
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
            klass.bind_template_callbacks();
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

            let obj = self.obj();

            // Bind properties to widgets
            obj.bind_property("aur-command", &self.aur_row.get(), "text")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                .build();

            obj.bind_property("remember-columns", &self.column_switch.get(), "active")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                .build();
            obj.bind_property("remember-sort", &self.sort_switch.get(), "active")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                .build();
        }
    }

    impl WidgetImpl for PreferencesWindow {}
    impl WindowImpl for PreferencesWindow {}
    impl AdwWindowImpl for PreferencesWindow {} 
    impl PreferencesWindowImpl for PreferencesWindow {}

    #[gtk::template_callbacks]
    impl PreferencesWindow {
        //-----------------------------------
        // Reset button signal handler
        //-----------------------------------
        #[template_callback]
        fn on_reset_button_clicked(&self) {
            let win = self.obj().root().unwrap().downcast::<gtk::Window>().unwrap();

            let reset_dialog = adw::MessageDialog::new(Some(&win), Some("Reset Preferences?"), Some("Reset all preferences to their default values."));

            reset_dialog.add_responses(&[("cancel", "_Cancel"), ("reset", "_Reset")]);
            reset_dialog.set_response_appearance("reset", adw::ResponseAppearance::Destructive);

            reset_dialog.connect_response(Some("reset"), clone!(@weak self as prefs => move |_, _| {
                prefs.aur_row.set_text("");
                prefs.column_switch.set_active(true);
                prefs.sort_switch.set_active(false);
            }));

            reset_dialog.present();
        }
    }
}

//------------------------------------------------------------------------------
// PUBLIC IMPLEMENTATION: PreferencesWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PreferencesWindow(ObjectSubclass<imp::PreferencesWindow>)
        @extends adw::Window, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl PreferencesWindow {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
