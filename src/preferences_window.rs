use std::cell::{Cell, RefCell};

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::clone;
use gtk::pango::FontDescription;

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
        #[template_child]
        pub font_expander: TemplateChild<adw::ExpanderRow>,
        #[template_child]
        pub font_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub font_row: TemplateChild<adw::ActionRow>,

        #[property(get, set)]
        aur_command: RefCell<String>,
        #[property(get, set)]
        remember_columns: Cell<bool>,
        #[property(get, set)]
        remember_sort: Cell<bool>,
        #[property(get, set)]
        custom_font: Cell<bool>,
        #[property(get, set)]
        monospace_font: RefCell<String>,
        #[property(get, set)]
        default_monospace_font: RefCell<String>,
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

            // Bind widget states
            self.font_expander.bind_property("expanded", &self.font_switch.get(), "active")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                .build();

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

            obj.bind_property("custom-font", &self.font_switch.get(), "active")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                .build();
            obj.bind_property("monospace-font", &self.font_row.get(), "title")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                .build();

            // Set AUR row tooltip
            self.aur_row.set_tooltip_markup(Some(
                "The command should return a list of AUR updates in the format:\n\n\
                <tt>package_name current_version -> new_version</tt>")
            );
        }
    }

    impl WidgetImpl for PreferencesWindow {}
    impl WindowImpl for PreferencesWindow {}
    impl AdwWindowImpl for PreferencesWindow {} 
    impl PreferencesWindowImpl for PreferencesWindow {}

    #[gtk::template_callbacks]
    impl PreferencesWindow {
        //-----------------------------------
        // Font button signal handler
        //-----------------------------------
        #[template_callback]
        fn on_font_reset_button_clicked(&self) {
            self.font_row.set_title(&self.obj().default_monospace_font());
        }

        #[template_callback]
        fn on_font_choose_button_clicked(&self) {
            let font_dialog = gtk::FontDialog::new();

            font_dialog.choose_font(
                Some(&*self.obj()),
                Some(&FontDescription::from_string(&self.font_row.title())),
                None::<&gio::Cancellable>,
                clone!(@weak self as prefs => move |result| {
                if let Ok(font_desc) = result {
                    prefs.font_row.set_title(&font_desc.to_string());
                }
            }));
        }

        //-----------------------------------
        // Reset button signal handler
        //-----------------------------------
        #[template_callback]
        fn on_reset_button_clicked(&self) {
            let reset_dialog = adw::MessageDialog::new(
                Some(&*self.obj()),
                Some("Reset Preferences?"),
                Some("Reset all preferences to their default values.")
            );

            reset_dialog.add_responses(&[("cancel", "_Cancel"), ("reset", "_Reset")]);
            reset_dialog.set_response_appearance("reset", adw::ResponseAppearance::Destructive);

            reset_dialog.connect_response(Some("reset"), clone!(@weak self as prefs => move |_, _| {
                prefs.aur_row.set_text("");
                prefs.column_switch.set_active(true);
                prefs.sort_switch.set_active(false);
                prefs.font_switch.set_active(true);
                prefs.font_row.set_title(&prefs.obj().default_monospace_font());
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
