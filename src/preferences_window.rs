use std::cell::{Cell, RefCell};

use gtk::{glib, gio};
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
        pub refresh_switchrow: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub aur_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub delay_spinrow: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub aur_menubutton: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub column_switchrow: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub sort_switchrow: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub reset_button: TemplateChild<gtk::Button>,

        #[property(get, set)]
        auto_refresh: Cell<bool>,
        #[property(get, set)]
        aur_command: RefCell<String>,
        #[property(get, set)]
        search_delay: Cell<f64>,
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
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PreferencesWindow {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
            obj.setup_actions();
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

        // Bind properties to widgets
        self.bind_property("auto_refresh", &imp.refresh_switchrow.get(), "active")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();
        self.bind_property("aur-command", &imp.aur_row.get(), "text")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();
        self.bind_property("search-delay", &imp.delay_spinrow.get(), "value")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();

        self.bind_property("remember-columns", &imp.column_switchrow.get(), "active")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();
        self.bind_property("remember-sort", &imp.sort_switchrow.get(), "active")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();

        // Set AUR row tooltip
        imp.aur_row.set_tooltip_markup(Some(
            "The command must return a list of AUR updates in the format:\n\n\
            <tt>package_name current_version -> new_version</tt>"
        ));
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        // Add AUR helper command action with parameter
        let aur_action = gio::SimpleAction::new("aur-cmd", Some(&String::static_variant_type()));

        aur_action.connect_activate(clone!(@weak self as window => move |_, param| {
            let param = param
                .expect("Must be a 'Variant'")
                .get::<String>()
                .expect("Must be a 'String'");

            let cmd = match param.as_str() {
                "paru" => "/usr/bin/paru -Qu --mode=ap",
                "pikaur" => "/usr/bin/pikaur -Qua 2>/dev/null",
                "trizen" => "/usr/bin/trizen -Qua --devel",
                "yay" => "/usr/bin/yay -Qua",
                _ => unreachable!()
            };

            window.set_aur_command(cmd);
        }));

        // Add action to prefs group
        let prefs_group = gio::SimpleActionGroup::new();

        self.insert_action_group("prefs", Some(&prefs_group));

        prefs_group.add_action(&aur_action);
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Preferences reset button clicked signal
        imp.reset_button.connect_clicked(clone!(@weak self as window => move |_| {
            let reset_dialog = adw::MessageDialog::new(
                Some(&window),
                Some("Reset Preferences?"),
                Some("Reset all preferences to their default values.")
            );

            reset_dialog.add_responses(&[("cancel", "_Cancel"), ("reset", "_Reset")]);
            reset_dialog.set_default_response(Some("reset"));

            reset_dialog.set_response_appearance("reset", adw::ResponseAppearance::Destructive);

            reset_dialog.choose(
                None::<&gio::Cancellable>,
                clone!(@weak window => move |response| {
                    if response == "reset" {
                        window.set_auto_refresh(true);
                        window.set_aur_command("");
                        window.set_search_delay(150.0);
                        window.set_remember_columns(true);
                        window.set_remember_sort(false);
                    }
                })
            );
        }));
    }
}
