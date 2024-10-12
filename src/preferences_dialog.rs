use std::cell::{Cell, RefCell};

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::clone;

use crate::search_bar::{SearchMode, SearchProp};
use crate::traits::EnumValueExt;

//------------------------------------------------------------------------------
// MODULE: PreferencesDialog
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PreferencesDialog)]
    #[template(resource = "/com/github/PacView/ui/preferences_dialog.ui")]
    pub struct PreferencesDialog {
        #[template_child]
        pub(super) auto_refresh_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) aur_command_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub(super) search_mode_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) search_prop_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) search_delay_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub(super) aur_menubutton: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub(super) remember_sort_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) reset_button: TemplateChild<adw::ButtonRow>,

        #[property(get, set)]
        auto_refresh: Cell<bool>,
        #[property(get, set)]
        aur_command: RefCell<String>,
        #[property(get, set, builder(SearchMode::default()))]
        search_mode: Cell<SearchMode>,
        #[property(get, set, builder(SearchProp::default()))]
        search_prop: Cell<SearchProp>,
        #[property(get, set)]
        search_delay: Cell<f64>,
        #[property(get, set)]
        remember_sort: Cell<bool>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesDialog {
        const NAME: &'static str = "PreferencesDialog";
        type Type = super::PreferencesDialog;
        type ParentType = adw::PreferencesDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PreferencesDialog {
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

    impl WidgetImpl for PreferencesDialog {}
    impl AdwDialogImpl for PreferencesDialog {}
    impl PreferencesDialogImpl for PreferencesDialog {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PreferencesDialog
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct PreferencesDialog(ObjectSubclass<imp::PreferencesDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PreferencesDialog {
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
        self.bind_property("auto-refresh", &imp.auto_refresh_row.get(), "active")
            .sync_create()
            .bidirectional()
            .build();
        self.bind_property("aur-command", &imp.aur_command_row.get(), "text")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("search-mode", &imp.search_mode_row.get(), "selected")
            .transform_to(|_, mode: SearchMode| Some(mode.value()))
            .transform_from(|_, index: u32| {
                Some(SearchMode::from_repr(index).unwrap_or_default())
            })
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("search-prop", &imp.search_prop_row.get(), "selected")
            .transform_to(|_, prop: SearchProp| Some(prop.value()))
            .transform_from(|_, index: u32| {
                Some(SearchProp::from_repr(index).unwrap_or_default())
            })
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("search-delay", &imp.search_delay_row.get(), "value")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("remember-sort", &imp.remember_sort_row.get(), "active")
            .sync_create()
            .bidirectional()
            .build();

        // Set AUR row tooltip
        imp.aur_command_row.set_tooltip_markup(Some(
            "The command must return a list of AUR updates in the format:\n\n\
            <tt>package_name current_version -> new_version</tt>"
        ));
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        // Add AUR helper command action with parameter
        let aur_action = gio::ActionEntry::builder("aur-cmd", )
            .parameter_type(Some(&String::static_variant_type()))
            .activate(clone!(
                #[weak(rename_to = dialog)] self,
                move |_, _, param| {
                    let param = param
                        .expect("Could not retrieve Variant")
                        .get::<String>()
                        .expect("Could not retrieve String from variant");

                    let cmd = match param.as_str() {
                        "paru" => "/usr/bin/paru -Qu --mode=ap",
                        "pikaur" => "/usr/bin/pikaur -Qua 2>/dev/null",
                        "trizen" => "/usr/bin/trizen -Qua --devel",
                        "yay" => "/usr/bin/yay -Qua",
                        _ => unreachable!()
                    };

                    dialog.set_aur_command(cmd);
                }
            ))
            .build();

        // Add action to prefs group
        let prefs_group = gio::SimpleActionGroup::new();

        self.insert_action_group("prefs", Some(&prefs_group));

        prefs_group.add_action_entries([aur_action]);
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Preferences reset button clicked signal
        imp.reset_button.connect_activated(clone!(
            #[weak(rename_to = dialog)] self,
            move |_| {
                let reset_dialog = adw::AlertDialog::builder()
                    .heading("Reset Preferences?")
                    .body("Reset all preferences to their default values.")
                    .default_response("reset")
                    .build();

                reset_dialog.add_responses(&[("cancel", "_Cancel"), ("reset", "_Reset")]);
                reset_dialog.set_response_appearance("reset", adw::ResponseAppearance::Destructive);

                reset_dialog.choose(
                    &dialog,
                    None::<&gio::Cancellable>,
                    clone!(
                        #[weak] dialog,
                        move |response| {
                            if response == "reset" {
                                dialog.set_auto_refresh(true);
                                dialog.set_aur_command("");
                                dialog.set_search_mode(SearchMode::default());
                                dialog.set_search_prop(SearchProp::default());
                                dialog.set_search_delay(150.0);
                                dialog.set_remember_sort(false);
                            }
                        }
                    )
                );
            }
        ));
    }
}

impl Default for PreferencesDialog {
    //-----------------------------------
    // Default constructor
    //-----------------------------------
    fn default() -> Self {
        Self::new()
    }
}
