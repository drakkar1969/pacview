use std::cell::{Cell, RefCell};

use gtk::{glib, gio, pango};
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
        pub aur_image: TemplateChild<gtk::Image>,
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
        #[template_child]
        pub font_reset_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub font_choose_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub reset_button: TemplateChild<gtk::Button>,

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

            obj.setup_widgets();
            obj.setup_signals();
        }
    }

    impl WidgetImpl for PreferencesWindow {}
    impl WindowImpl for PreferencesWindow {}
    impl AdwWindowImpl for PreferencesWindow {} 
    impl PreferencesWindowImpl for PreferencesWindow {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PreferencesWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PreferencesWindow(ObjectSubclass<imp::PreferencesWindow>)
        @extends adw::Window, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl PreferencesWindow {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind widget states
        imp.font_expander.bind_property("expanded", &imp.font_switch.get(), "active")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();

        // Bind properties to widgets
        self.bind_property("aur-command", &imp.aur_row.get(), "text")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();

        self.bind_property("remember-columns", &imp.column_switch.get(), "active")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();
        self.bind_property("remember-sort", &imp.sort_switch.get(), "active")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();

        self.bind_property("custom-font", &imp.font_switch.get(), "active")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();
        self.bind_property("monospace-font", &imp.font_row.get(), "title")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();

        // Set AUR image tooltip
        imp.aur_image.set_tooltip_markup(Some(
            "The command must return a list of AUR updates in the format:\n\n\
            <tt>package_name current_version -> new_version</tt>")
        );
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Font reset button clicked signal
        imp.font_reset_button.connect_clicked(clone!(@weak self as obj => move |_| {
            obj.set_monospace_font(obj.default_monospace_font());
        }));

        // Font choose button clicked signal
        imp.font_choose_button.connect_clicked(clone!(@weak self as obj => move |_| {
            let font_dialog = gtk::FontDialog::new();

            font_dialog.set_title("Select Font");

            font_dialog.choose_font(
                Some(&obj),
                Some(&pango::FontDescription::from_string(&obj.monospace_font())),
                None::<&gio::Cancellable>,
                clone!(@weak obj => move |result| {
                    if let Ok(font_desc) = result {
                        obj.set_monospace_font(font_desc.to_string());
                    }
                })
            );
        }));

        // Preferences reset button clicked signal
        imp.reset_button.connect_clicked(clone!(@weak self as obj, @weak imp => move |_| {
            let reset_dialog = adw::MessageDialog::new(
                Some(&obj),
                Some("Reset Preferences?"),
                Some("Reset all preferences to their default values.")
            );

            reset_dialog.add_responses(&[("cancel", "_Cancel"), ("reset", "_Reset")]);
            reset_dialog.set_default_response(Some("reset"));

            reset_dialog.set_response_appearance("reset", adw::ResponseAppearance::Destructive);

            reset_dialog.choose(
                None::<&gio::Cancellable>,
                clone!(@weak obj=> move |response| {
                    if response == "reset" {
                        obj.set_aur_command("");
                        obj.set_remember_columns(true);
                        obj.set_remember_sort(false);
                        obj.set_custom_font(true);
                        obj.set_monospace_font(obj.default_monospace_font());
                    }
                })
            );
        }));
    }
}
