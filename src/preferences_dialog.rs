use std::cell::{Cell, RefCell};

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::clone;

use strum::FromRepr;

use crate::APP_ID;
use crate::search_bar::{SearchMode, SearchProp};
use crate::enum_traits::EnumExt;

//------------------------------------------------------------------------------
// ENUM: ColorScheme
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum, FromRepr)]
#[repr(u32)]
#[enum_type(name = "ColorScheme")]
pub enum ColorScheme {
    #[default]
    Default,
    Light,
    Dark,
}

impl EnumExt for ColorScheme {
}

//------------------------------------------------------------------------------
// MODULE: PreferencesDialog
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PreferencesDialog)]
    #[template(resource = "/com/github/PacView/ui/preferences_dialog.ui")]
    pub struct PreferencesDialog {
        #[template_child]
        pub(super) color_scheme_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) sidebar_width_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub(super) infopane_width_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub(super) aur_command_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub(super) aur_database_download_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) aur_database_age_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub(super) auto_refresh_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) remember_sort_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) search_mode_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) search_prop_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) search_delay_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub(super) property_max_lines_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub(super) property_line_spacing_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub(super) underline_links_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) reset_button: TemplateChild<adw::ButtonRow>,

        #[property(get, set, builder(ColorScheme::default()))]
        color_scheme: Cell<ColorScheme>,
        #[property(get, set)]
        sidebar_width: Cell<f64>,
        #[property(get, set)]
        infopane_width: Cell<f64>,
        #[property(get, set)]
        aur_update_command: RefCell<String>,
        #[property(get, set)]
        aur_database_download: Cell<bool>,
        #[property(get, set)]
        aur_database_age: Cell<f64>,
        #[property(get, set)]
        auto_refresh: Cell<bool>,
        #[property(get, set)]
        remember_sort: Cell<bool>,
        #[property(get, set, builder(SearchMode::default()))]
        search_mode: Cell<SearchMode>,
        #[property(get, set, builder(SearchProp::default()))]
        search_prop: Cell<SearchProp>,
        #[property(get, set)]
        search_delay: Cell<f64>,
        #[property(get, set)]
        property_line_spacing: Cell<f64>,
        #[property(get, set)]
        property_max_lines: Cell<f64>,
        #[property(get, set)]
        underline_links: Cell<bool>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesDialog {
        const NAME: &'static str = "PreferencesDialog";
        type Type = super::PreferencesDialog;
        type ParentType = adw::PreferencesDialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            //---------------------------------------
            // Add class actions
            //---------------------------------------
            // AUR helper command action with parameter
            klass.install_action("prefs.aur-cmd",
                Some(&String::static_variant_type()),
                |dialog, _, param| {
                    let param = param
                        .expect("Failed to retrieve Variant")
                        .get::<String>()
                        .expect("Failed to retrieve String from variant");

                    let cmd = match param.as_str() {
                        "paru" => "/usr/bin/paru -Qu --mode=ap",
                        "pikaur" => "/usr/bin/pikaur -Qua 2>/dev/null",
                        "trizen" => "/usr/bin/trizen -Qua --devel",
                        "yay" => "/usr/bin/yay -Qua",
                        _ => unreachable!()
                    };

                    dialog.set_aur_update_command(cmd);
                }
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PreferencesDialog {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
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
        @extends adw::PreferencesDialog, adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl PreferencesDialog {
    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind properties to widgets
        self.bind_property("color-scheme", &imp.color_scheme_row.get(), "selected")
            .transform_to(|_, scheme: ColorScheme| Some(scheme.value()))
            .transform_from(|_, index: u32| {
                Some(ColorScheme::from_repr(index).unwrap_or_default())
            })
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("sidebar-width", &imp.sidebar_width_row.get(), "value")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("infopane-width", &imp.infopane_width_row.get(), "value")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("aur-update-command", &imp.aur_command_row.get(), "text")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("aur-database-download", &imp.aur_database_download_row.get(), "active")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("aur-database-download", &imp.aur_database_age_row.get(), "sensitive")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("aur-database-age", &imp.aur_database_age_row.get(), "value")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("auto-refresh", &imp.auto_refresh_row.get(), "active")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("remember-sort", &imp.remember_sort_row.get(), "active")
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

        self.bind_property("property-line_spacing", &imp.property_line_spacing_row.get(), "value")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("property-max-lines", &imp.property_max_lines_row.get(), "value")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("underline-links", &imp.underline_links_row.get(), "active")
            .sync_create()
            .bidirectional()
            .build();
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Color scheme row selected property notify signal
        imp.color_scheme_row.connect_selected_notify(clone!(
            #[weak(rename_to = dialog)] self,
            move |row| {
                let color_scheme = match ColorScheme::from_repr(row.selected())
                    .unwrap_or_default()
                {
                    ColorScheme::Default => adw::ColorScheme::PreferLight,
                    ColorScheme::Light => adw::ColorScheme::ForceLight,
                    ColorScheme::Dark => adw::ColorScheme::ForceDark,
                };

                let style_manager = adw::StyleManager::for_display(&dialog.display());

                style_manager.set_color_scheme(color_scheme);
            }
        ));

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
                    move |response| {
                        if response == "reset" {
                            let settings = gio::Settings::new(APP_ID);

                            settings.reset("color-scheme");
                            settings.reset("sidebar-width");
                            settings.reset("infopane-width");
                            settings.reset("aur-update-command");
                            settings.reset("aur-database-download");
                            settings.reset("aur-database-age");
                            settings.reset("auto-refresh");
                            settings.reset("remember-sort");
                            settings.reset("search-mode");
                            settings.reset("search-prop");
                            settings.reset("search-delay");
                            settings.reset("property-max-lines");
                            settings.reset("property-line-spacing");
                            settings.reset("underline-links");
                        }
                    }
                );
            }
        ));
    }
}

impl Default for PreferencesDialog {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
