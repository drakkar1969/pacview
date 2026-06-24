use std::cell::{Cell, RefCell};
use std::fmt::Write as _;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::{clone, Propagation};
use gdk::{Key, ModifierType};

use strum::AsRefStr;

use crate::{
    pkg_object::PkgObject,
    backup_object::{BackupObject, BackupStatus},
    utils::{Paths, Pacman, AppInfoExt}
};

//------------------------------------------------------------------------------
// ENUM: BackupSearchMode
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum, AsRefStr)]
#[strum(serialize_all = "lowercase")]
#[repr(u32)]
#[enum_type(name = "BackupSearchMode")]
pub enum BackupSearchMode {
    #[default]
    All,
    Packages,
    Files,
}

//------------------------------------------------------------------------------
// MODULE: BackupWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::BackupWindow)]
    #[template(resource = "/com/github/PacView/ui/backup_window.ui")]
    pub struct BackupWindow {
        #[template_child]
        pub(super) search_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) status_dropdown: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub(super) compare_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,

        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) filter_model: TemplateChild<gtk::FilterListModel>,
        #[template_child]
        pub(super) section_sort_model: TemplateChild<gtk::SortListModel>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) search_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub(super) status_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub(super) section_sorter: TemplateChild<gtk::StringSorter>,

        #[template_child]
        pub(super) footer_label: TemplateChild<gtk::Label>,

        #[property(get, set)]
        is_loaded: Cell<bool>,
        #[property(get, set, builder(BackupSearchMode::default()))]
        search_mode: Cell<BackupSearchMode>,
        #[property(get, set)]
        can_compare: Cell<bool>,

        pub(super) search_term: RefCell<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for BackupWindow {
        const NAME: &'static str = "BackupWindow";
        type Type = super::BackupWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            BackupStatus::ensure_type();
            BackupObject::ensure_type();

            klass.bind_template();

            // Install actions
            Self::install_actions(klass);

            // Add key bindings
            Self::bind_shortcuts(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for BackupWindow {
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

    impl WidgetImpl for BackupWindow {}
    impl WindowImpl for BackupWindow {}
    impl AdwWindowImpl for BackupWindow {}

    impl BackupWindow {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Search mode property action
            klass.install_property_action("search.set-mode", "search-mode");

            // Compare action
            klass.install_action_async("backup.compare", None, async |window, _, _| {
                if let Some(backup_file) = window.imp().selection.selected_item()
                    .and_downcast::<BackupObject>() {
                        let _ = backup_file.compare_with_original().await;
                    }
            });

            // Open action
            klass.install_action_async("backup.open", None, async |window, _, _| {
                if let Some(backup_file) = window.imp().selection.selected_item()
                    .and_downcast::<BackupObject>() {
                        let path = Pacman::config().root_dir.clone() + &backup_file.path();

                        AppInfoExt::open_with_default_app(&path).await;
                    }
            });

            // Copy action
            klass.install_action("backup.copy", None, |window, _, _| {
                let mut package = String::new();
                let mut output = String::from("## Backup Files\n|Filename|Status|\n|---|---|\n");

                for backup in window.imp().selection.iter::<glib::Object>()
                    .flatten()
                    .filter_map(|item| item.downcast::<BackupObject>().ok()) {
                        let backup_package = backup.package();

                        if backup_package != package {
                            writeln!(output, "|**{backup_package}**||").unwrap();

                            package = backup_package;
                        }

                        writeln!(output, "|{path}|{status}|",
                            path=backup.path(),
                            status=backup.status_text()
                        ).unwrap();
                    }

                window.clipboard().set_text(&output);
            });
        }

        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Close window binding
            klass.add_binding_action(Key::Escape, ModifierType::NO_MODIFIER_MASK, "window.close");

            // Find key binding
            klass.add_binding(Key::F, ModifierType::CONTROL_MASK, |window| {
                window.imp().search_bar.set_search_mode(true);

                Propagation::Stop
            });

            // Compare key binding
            klass.add_binding_action(Key::P, ModifierType::CONTROL_MASK, "backup.compare");

            // Open key binding
            klass.add_binding_action(Key::O, ModifierType::CONTROL_MASK, "backup.open");

            // Copy key binding
            klass.add_binding_action(Key::C, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "backup.copy");

            // Status key bindings
            klass.add_binding(Key::A, ModifierType::ALT_MASK, |window| {
                window.imp().status_dropdown.set_selected(BackupStatus::All as u32);

                Propagation::Stop
            });

            klass.add_binding(Key::M, ModifierType::ALT_MASK, |window| {
                window.imp().status_dropdown.set_selected(BackupStatus::Modified as u32);

                Propagation::Stop
            });

            klass.add_binding(Key::U, ModifierType::ALT_MASK, |window| {
                window.imp().status_dropdown.set_selected(BackupStatus::Unmodified as u32);

                Propagation::Stop
            });

            klass.add_binding(Key::L, ModifierType::ALT_MASK, |window| {
                window.imp().status_dropdown.set_selected(BackupStatus::Locked as u32);

                Propagation::Stop
            });
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: BackupWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct BackupWindow(ObjectSubclass<imp::BackupWindow>)
        @extends adw::Window, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl BackupWindow {
    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Is loaded property notify signal
        self.connect_is_loaded_notify(|window| {
            let imp = window.imp();

            imp.stack.set_visible_child_name(
                if window.is_loaded() {
                    if imp.section_sort_model.n_items() == 0 { "empty" } else { "view" }
                } else {
                    "loading"
                }
            );
        });

        // Search entry search changed signal
        imp.search_entry.connect_search_changed(clone!(
            #[weak] imp,
            move |_| {
                let term = imp.search_entry.text().trim().to_lowercase();

                imp.search_term.replace(term);

                imp.search_filter.changed(gtk::FilterChange::Different);
            }
        ));

        // Search mode property notify signal
        self.connect_search_mode_notify(|window| {
            let imp = window.imp();

            let search_mode = window.search_mode();

            if search_mode == BackupSearchMode::All {
                imp.search_entry.set_placeholder_text(Some("Search all"));
            } else {
                imp.search_entry.set_placeholder_text(Some(&format!("Search for {}", search_mode.as_ref())));
            }

            imp.search_filter.changed(gtk::FilterChange::Different);
        });

        // Status dropdown selected property notify signal
        imp.status_dropdown.connect_selected_item_notify(clone!(
            #[weak] imp,
            move |_| {
                imp.status_filter.changed(gtk::FilterChange::Different);

                imp.view.grab_focus();
            }
        ));

        // Section sort model items changed signal
        imp.section_sort_model.connect_items_changed(clone!(
            #[weak(rename_to = window)] self,
            move |sort_model, _, _, _| {
                let imp = window.imp();

                let n_items = sort_model.n_items();
                let mut n_sections = 0;

                if n_items != 0 {
                    let mut index = 0;

                    while index < n_items {
                        let (_, end) = sort_model.section(index);

                        n_sections += 1;
                        index = end;
                    }
                }

                imp.stack.set_visible_child_name(
                    if window.is_loaded() {
                        if n_items == 0 { "empty" } else { "view" }
                    } else {
                        "loading"
                    }
                );

                imp.footer_label.set_label(&format!("{n_items} files in {n_sections} package{}", if n_sections == 1 { "" } else { "s" }));

                let status = imp.selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .map_or(BackupStatus::Locked, |backup| backup.status());

                window.action_set_enabled("backup.compare", window.can_compare() && status == BackupStatus::Modified);
                window.action_set_enabled("backup.open", status != BackupStatus::Locked);
                window.action_set_enabled("backup.copy", n_items > 0);
            }
        ));

        // Selection selected item property notify signal
        imp.selection.connect_selected_item_notify(clone!(
            #[weak(rename_to = window)] self,
            move |selection| {
                let status = selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .map_or(BackupStatus::Locked, |backup| backup.status());

                window.action_set_enabled("backup.compare", window.can_compare() && status == BackupStatus::Modified);
                window.action_set_enabled("backup.open", status != BackupStatus::Locked);
            }
        ));

        // Column view activate signal
        imp.view.connect_activate(clone!(
            #[weak(rename_to = window)] self,
            move |_, _| {
                window.activate_action("backup.open", None).unwrap();
            }
        ));
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set search bar key capture widget and connect entry
        imp.search_bar.set_key_capture_widget(Some(&imp.view.get()));
        imp.search_bar.connect_entry(&imp.search_entry.get());

        // Bind search button state to search bar visibility
        imp.search_button.bind_property("active", &imp.search_bar.get(), "search-mode-enabled")
            .bidirectional()
            .sync_create()
            .build();

        // Bind can compare property to compare button visibility
        self.bind_property("can-compare", &imp.compare_button.get(), "visible")
            .sync_create()
            .build();

        // Set search filter function
        imp.search_filter.set_filter_func(clone!(
            #[weak(rename_to = window)] self,
            #[upgrade_or] false,
            move |item| {
                let search_term = window.imp().search_term.borrow();

                if search_term.is_empty() {
                    return true;
                }

                let obj = item
                    .downcast_ref::<BackupObject>()
                    .expect("Failed to downcast to 'BackupObject'");

                let is_match = |prop: &str| -> bool {
                    prop.as_bytes()
                        .windows(search_term.len())
                        .any(|window| window.eq_ignore_ascii_case(search_term.as_bytes()))
                };

                match window.search_mode() {
                    BackupSearchMode::All => {
                        is_match(&obj.path()) || is_match(&obj.package())
                    },
                    BackupSearchMode::Packages => {
                        is_match(&obj.package())
                    },
                    BackupSearchMode::Files => {
                        is_match(&obj.path())
                    },
                }
            }
        ));

        imp.status_filter.set_filter_func(clone!(
            #[weak] imp,
            #[upgrade_or] false,
            move |item| {
                let obj = item
                    .downcast_ref::<BackupObject>()
                    .expect("Failed to downcast to 'BackupObject'");

                let status = BackupStatus::from_repr(imp.status_dropdown.selected())
                    .unwrap_or_default();

                if status == BackupStatus::All {
                    true
                } else {
                    obj.status() == status
                }
            }
        ));

        // Set backup compare button visibility
        self.set_can_compare(Paths::paccat().is_ok() && Paths::meld().is_ok());

        // Set initial focus on view
        imp.view.grab_focus();
    }

    //---------------------------------------
    // Populate window
    //---------------------------------------
    fn populate(&self, pkg_model: &gio::ListStore) {
        let imp = self.imp();

        // Get backup list
        glib::spawn_future_local(clone!(
            #[weak] imp,
            #[weak] pkg_model,
            async move {
                let backup_list: Vec<BackupObject> = pkg_model.iter::<PkgObject>()
                    .flatten()
                    .filter(PkgObject::is_installed)
                    .flat_map(|pkg| {
                        let pkg_name = pkg.name();

                        pkg.backup().iter()
                            .map(|backup| BackupObject::new(backup, &pkg_name))
                            .collect::<Vec<BackupObject>>()
                    })
                    .collect();

                // Populate column view
                imp.model.splice(0, imp.model.n_items(), &backup_list);

                // Set status dropdown selected item
                imp.status_dropdown.set_selected(0);
            }
        ));
    }

    //---------------------------------------
    // Show window
    //---------------------------------------
    pub fn show(&self, pkg_model: &gio::ListStore) {
        self.present();

        glib::idle_add_local_once(clone!(
            #[weak(rename_to = window)] self,
            #[weak] pkg_model,
            move || {
                if !window.is_loaded() {
                    window.populate(&pkg_model);

                    window.set_is_loaded(true);
                }
            }
        ));
    }
}

impl Default for BackupWindow {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
