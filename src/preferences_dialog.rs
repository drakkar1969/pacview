use std::cell::{Cell, RefCell};
use std::fs;

use gtk::{gio, glib, pango};
use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::clone;

use walkdir::WalkDir;

use crate::{
    APP_ID,
    search_bar::SearchProp,
    utils::{Paths, StyleSchemes, AppInfoExt},
};

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
        pub(super) aur_database_download_row: TemplateChild<adw::ExpanderRow>,
        #[template_child]
        pub(super) aur_database_download_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub(super) aur_database_age_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub(super) enable_pkgbuild_repos_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) auto_refresh_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) remember_sort_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) remember_grouping_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) search_prop_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) search_exact_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) infopane_width_fraction_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub(super) property_max_lines_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub(super) property_line_spacing_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub(super) underline_links_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) pkgbuild_style_scheme_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) pkgbuild_use_system_font_row: TemplateChild<adw::ExpanderRow>,
        #[template_child]
        pub(super) pkgbuild_use_system_font_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub(super) pkgbuild_custom_font_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) open_cache_button: TemplateChild<adw::ButtonRow>,
        #[template_child]
        pub(super) clear_cache_button: TemplateChild<adw::ButtonRow>,
        #[template_child]
        pub(super) reset_button: TemplateChild<adw::ButtonRow>,

        #[property(get, set)]
        aur_database_download: Cell<bool>,
        #[property(get, set)]
        aur_database_age: Cell<f64>,
        #[property(get, set)]
        enable_pkgbuild_repos: Cell<bool>,
        #[property(get, set)]
        auto_refresh: Cell<bool>,
        #[property(get, set)]
        remember_sort: Cell<bool>,
        #[property(get, set)]
        remember_grouping: Cell<bool>,
        #[property(get, set, builder(SearchProp::default()))]
        search_prop: Cell<SearchProp>,
        #[property(get, set)]
        search_exact: Cell<bool>,
        #[property(get, set)]
        property_line_spacing: Cell<f64>,
        #[property(get, set)]
        infopane_width_fraction: Cell<f64>,
        #[property(get, set)]
        property_max_lines: Cell<f64>,
        #[property(get, set)]
        underline_links: Cell<bool>,
        #[property(get, set)]
        pkgbuild_style_scheme: RefCell<String>,
        #[property(get, set)]
        pkgbuild_use_system_font: Cell<bool>,
        #[property(get, set)]
        pkgbuild_custom_font: RefCell<String>,
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

            // Install actions
            Self::install_actions(klass);
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

            obj.setup_signals();
            obj.setup_widgets();
        }
    }

    impl WidgetImpl for PreferencesDialog {}
    impl AdwDialogImpl for PreferencesDialog {}
    impl PreferencesDialogImpl for PreferencesDialog {}

    impl PreferencesDialog {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            klass.install_action("prefs.pkgbuild-custom-font", None, |dialog, _, _| {
                let font_dialog = gtk::FontDialog::builder()
                    .modal(true)
                    .title("Select Font")
                    .build();

                font_dialog.choose_font(
                    dialog.root().and_downcast_ref::<gtk::Window>(),
                    Some(&pango::FontDescription::from_string(&dialog.pkgbuild_custom_font())),
                    None::<&gio::Cancellable>,
                    clone!(
                        #[weak] dialog,
                        move |response| {
                            if let Ok(font_desc) = response {
                                dialog.set_pkgbuild_custom_font(font_desc.to_string());
                            }
                        }
                    )
                );
            });

            // Open cache action
            klass.install_action_async("prefs.open-cache", None, async |_, _, _| {
                let path = Paths::cache_dir().display().to_string();

                AppInfoExt::open_with_default_app(&path).await;
            });

            // Clear cache action
            klass.install_action("prefs.clear-cache", None, |dialog, _, _| {
                let clear_dialog = adw::AlertDialog::builder()
                    .heading("Clear Cache Folder?")
                    .body("Delete all files and folders in the PacView cache folder.")
                    .default_response("clear")
                    .build();

                clear_dialog.add_responses(&[("cancel", "_Cancel"), ("clear", "Clea_r")]);
                clear_dialog.set_response_appearance("clear", adw::ResponseAppearance::Destructive);

                clear_dialog.choose(
                    Some(dialog),
                    None::<&gio::Cancellable>,
                    move |response| {
                        if response == "clear" {
                            for entry in WalkDir::new(Paths::cache_dir())
                                .min_depth(1)
                                .max_depth(1)
                                .into_iter()
                                .flatten() {
                                    let path = entry.path();

                                    if let Ok(metadata) = path.metadata() {
                                        let _ = if metadata.file_type().is_dir() {
                                            fs::remove_dir_all(path)
                                        } else {
                                            fs::remove_file(path)
                                        };
                                    }
                                }
                        }
                    }
                );
            });

            // Reset preferences action
            klass.install_action("prefs.reset-preferences", None, |dialog, _, _| {
                let reset_dialog = adw::AlertDialog::builder()
                    .heading("Reset Preferences?")
                    .body("Reset all preferences to their default values.")
                    .default_response("reset")
                    .build();

                reset_dialog.add_responses(&[("cancel", "_Cancel"), ("reset", "_Reset")]);
                reset_dialog.set_response_appearance("reset", adw::ResponseAppearance::Destructive);

                reset_dialog.choose(
                    Some(dialog),
                    None::<&gio::Cancellable>,
                    move |response| {
                        if response == "reset" {
                            let settings = gio::Settings::new(APP_ID);

                            settings.reset("aur-database-download");
                            settings.reset("aur-database-age");
                            settings.reset("enable-pkgbuild-repos");
                            settings.reset("auto-refresh");
                            settings.reset("remember-sort");
                            settings.reset("remember-grouping");
                            settings.reset("search-prop");
                            settings.reset("search-exact");
                            settings.reset("infopane-width-fraction");
                            settings.reset("property-max-lines");
                            settings.reset("property-line-spacing");
                            settings.reset("underline-links");
                            settings.reset("pkgbuild-style-scheme");
                            settings.reset("pkgbuild-use-system-font");
                            settings.reset("pkgbuild-custom-font");
                        }
                    }
                );
            });
        }
    }
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
    // Populate style schemes helper function
    //---------------------------------------
    fn populate_style_schemes(&self, style_manager: &adw::StyleManager) {
        if let Some(model) = self.imp().pkgbuild_style_scheme_row.model()
            .and_downcast_ref::<gio::ListStore>() {
                let schemes = StyleSchemes::schemes(style_manager.is_dark());

                model.splice(0, model.n_items(), &schemes);

                self.notify_pkgbuild_style_scheme();
            }
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        // System color scheme signal
        let style_manager = adw::StyleManager::for_display(&self.display());

        style_manager.connect_dark_notify(clone!(
            #[weak(rename_to = dialog)] self,
            move |style_manager| {
                dialog.populate_style_schemes(style_manager);
            }
        ));
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Populate PKGBUILD style scheme combo row
        let style_manager = adw::StyleManager::for_display(&self.display());

        self.populate_style_schemes(&style_manager);

        // Bind properties to widgets
        self.bind_property("aur-database-download", &imp.aur_database_download_row.get(), "expanded")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("aur-database-download", &imp.aur_database_download_switch.get(), "active")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("aur-database-age", &imp.aur_database_age_row.get(), "value")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("enable-pkgbuild-repos", &imp.enable_pkgbuild_repos_row.get(), "active")
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

        self.bind_property("remember-grouping", &imp.remember_grouping_row.get(), "active")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("search-prop", &imp.search_prop_row.get(), "selected")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("search-exact", &imp.search_exact_row.get(), "active")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("infopane-width-fraction", &imp.infopane_width_fraction_row.get(), "value")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("property-max-lines", &imp.property_max_lines_row.get(), "value")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("property-line_spacing", &imp.property_line_spacing_row.get(), "value")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("underline-links", &imp.underline_links_row.get(), "active")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("pkgbuild-style-scheme", &imp.pkgbuild_style_scheme_row.get(), "selected")
            .transform_to(|binding, id: String| {
                binding.target()
                    .and_downcast::<adw::ComboRow>()?
                    .model()?
                    .iter::<sourceview5::StyleScheme>()
                    .flatten()
                    .position(|scheme| StyleSchemes::scheme_matches_id(&scheme, &id))
                    .or(Some(0))
                    .map(|index| index as u32)
            })
            .transform_from(|binding, _: u32| {
                binding.target()
                    .and_downcast::<adw::ComboRow>()?
                    .selected_item()
                    .and_downcast::<sourceview5::StyleScheme>()
                    .map(|scheme| scheme.id())
                    .or_else(|| Some("".into()))
            })
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("pkgbuild-use-system-font", &imp.pkgbuild_use_system_font_row.get(), "expanded")
            .invert_boolean()
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("pkgbuild-use-system-font", &imp.pkgbuild_use_system_font_switch.get(), "active")
            .sync_create()
            .bidirectional()
            .build();

        self.bind_property("pkgbuild-custom-font", &imp.pkgbuild_custom_font_row.get(), "subtitle")
            .sync_create()
            .bidirectional()
            .build();
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
