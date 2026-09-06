use std::cell::{Cell, RefCell};
use std::fmt::Write as _;
use std::io;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::{clone, Propagation};
use gdk::{Key, ModifierType};

use strum::{EnumIter, IntoEnumIterator, AsRefStr};
use tokio_util::sync::CancellationToken;

use crate::{
    pkg_object::PkgObject,
    backup_object::{BackupObject, BackupStatus},
    tokio_manager::TokioManager,
    utils::{Paths, Pacman, AppInfoExt, TokioCommand}
};

//------------------------------------------------------------------------------
// ENUM: BackupSearchMode
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum, EnumIter, AsRefStr)]
#[repr(u32)]
#[enum_type(name = "BackupSearchMode")]
pub enum BackupSearchMode {
    #[default]
    All,
    Packages,
    Files,
}

//------------------------------------------------------------------------------
// MODULE: BackupDialog
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::BackupDialog)]
    #[template(resource = "/com/github/PacView/ui/backup_dialog.ui")]
    pub struct BackupDialog {
        #[template_child]
        pub(super) search_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) compare_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) search_mode_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) section_sort_model: TemplateChild<gtk::SortListModel>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) search_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub(super) status_filter: TemplateChild<gtk::CustomFilter>,

        #[template_child]
        pub(super) count_label: TemplateChild<gtk::Label>,

        #[property(get, set)]
        is_loaded: Cell<bool>,
        #[property(get, set, builder(BackupSearchMode::default()))]
        search_mode: Cell<BackupSearchMode>,
        #[property(get, set, construct, builder(BackupStatus::All))]
        filter: Cell<BackupStatus>,
        #[property(get, set)]
        can_compare: Cell<bool>,
        #[property(get, set)]
        comparing: Cell<bool>,

        pub(super) search_term: RefCell<String>,

        pub(super) compare_cancel_token: RefCell<Option<CancellationToken>>
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for BackupDialog {
        const NAME: &'static str = "BackupDialog";
        type Type = super::BackupDialog;
        type ParentType = adw::Dialog;

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
    impl ObjectImpl for BackupDialog {
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

    impl WidgetImpl for BackupDialog {}
    impl AdwDialogImpl for BackupDialog {}

    impl BackupDialog {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Search mode property action
            klass.install_property_action("search.set-mode", "search-mode");

            // Cycle search mode action
            klass.install_action("search.cycle-mode", None, |dialog, _, _| {
                let new_mode = BackupSearchMode::iter().cycle()
                    .skip_while(|&mode| mode != dialog.search_mode())
                    .nth(1)
                    .expect("Failed to get 'BackupSearchMode'");

                dialog.set_search_mode(new_mode);
            });

            // Reverse cycle search mode action
            klass.install_action("search.reverse-cycle-mode", None, |dialog, _, _| {
                let new_mode = BackupSearchMode::iter().rev().cycle()
                    .skip_while(|&mode| mode != dialog.search_mode())
                    .nth(1)
                    .expect("Failed to get 'BackupSearchMode'");

                dialog.set_search_mode(new_mode);
            });

            // Filter property action
            klass.install_property_action("backup.filter", "filter");

            // Compare action
            klass.install_action_async("backup.compare", None, async |dialog, _, _| {
                dialog.cancel_compare();

                if !dialog.comparing() {
                    let backup_file = dialog.imp().selection.selected_item();

                    if let Some(file) = backup_file
                        .and_downcast::<BackupObject>() {
                            dialog.set_comparing(true);

                            let _ = dialog.compare_with_original(&file).await;

                            dialog.set_comparing(false);
                        }
                }
            });

            // Open action
            klass.install_action_async("backup.open", None, async |dialog, _, _| {
                if let Some(backup_file) = dialog.imp().selection.selected_item()
                    .and_downcast::<BackupObject>() {
                        let path = Pacman::config().read().unwrap().root_dir.clone() +
                            &backup_file.path();

                        AppInfoExt::open_with_default_app(&path).await;
                    }
            });

            // Copy action
            klass.install_action("backup.copy", None, |dialog, _, _| {
                let mut package = String::new();
                let mut output = String::from("## Backup Files\n|Filename|Status|\n|---|---|\n");

                for backup in dialog.imp().selection.iter::<glib::Object>()
                    .filter_map(|item| item.ok().and_downcast::<BackupObject>()) {
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

                dialog.clipboard().set_text(&output);
            });
        }

        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Find key binding
            klass.add_binding(Key::F, ModifierType::CONTROL_MASK, |dialog| {
                dialog.imp().search_bar.set_search_mode(true);

                Propagation::Stop
            });

            // Cycle search mode key bindings
            klass.add_binding_action(Key::M, ModifierType::CONTROL_MASK, "search.cycle-mode");
            klass.add_binding_action(Key::M, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "search.reverse-cycle-mode");

            // Compare key binding
            klass.add_binding_action(Key::P, ModifierType::CONTROL_MASK, "backup.compare");

            // Open key binding
            klass.add_binding_action(Key::O, ModifierType::CONTROL_MASK, "backup.open");

            // Copy key binding
            klass.add_binding_action(Key::C, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "backup.copy");

            // Status key bindings
            klass.add_binding(Key::A, ModifierType::ALT_MASK, |dialog| {
                dialog.set_filter(BackupStatus::All);

                Propagation::Stop
            });

            klass.add_binding(Key::M, ModifierType::ALT_MASK, |dialog| {
                dialog.set_filter(BackupStatus::Modified);

                Propagation::Stop
            });

            klass.add_binding(Key::U, ModifierType::ALT_MASK, |dialog| {
                dialog.set_filter(BackupStatus::Unmodified);

                Propagation::Stop
            });

            klass.add_binding(Key::L, ModifierType::ALT_MASK, |dialog| {
                dialog.set_filter(BackupStatus::Locked);

                Propagation::Stop
            });
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: BackupDialog
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct BackupDialog(ObjectSubclass<imp::BackupDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl BackupDialog {
    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Search bar search mode enabled signal
        imp.search_bar.connect_search_mode_enabled_notify(clone!(
            #[weak] imp,
            move |bar| {
                if !bar.is_search_mode() {
                    imp.view.grab_focus();
                }
            }
        ));

        // Search entry search changed signal
        imp.search_entry.connect_search_changed(clone!(
            #[weak] imp,
            move |entry| {
                imp.search_term.replace(entry.text().trim().to_lowercase());

                imp.search_filter.changed(gtk::FilterChange::Different);
            }
        ));

        // Search mode property notify signal
        self.connect_search_mode_notify(|dialog| {
            let imp = dialog.imp();

            imp.search_mode_label.set_label(dialog.search_mode().as_ref());

            imp.search_filter.changed(gtk::FilterChange::Different);
        });

        // Filter property notify signal
        self.connect_filter_notify(|dialog| {
            dialog.imp().status_filter.changed(gtk::FilterChange::Different);
        });

        // Section sort model items changed signal
        imp.section_sort_model.connect_items_changed(clone!(
            #[weak(rename_to = dialog)] self,
            move |sort_model, _, _, _| {
                let imp = dialog.imp();

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
                    if dialog.is_loaded() {
                        if n_items == 0 { "empty" } else { "view" }
                    } else {
                        "loading"
                    }
                );

                imp.count_label.set_label(&format!("{n_items} file{} in {n_sections} package{}",
                    if n_items == 1 { "" } else { "s" },
                    if n_sections == 1 { "" } else { "s" }
                ));

                let status = imp.selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .map_or(BackupStatus::Locked, |backup| backup.status());

                dialog.action_set_enabled("backup.compare", dialog.can_compare() && status == BackupStatus::Modified);
                dialog.action_set_enabled("backup.open", status != BackupStatus::Locked);
                dialog.action_set_enabled("backup.copy", n_items > 0);
            }
        ));

        // Selection selected item property notify signal
        imp.selection.connect_selected_item_notify(clone!(
            #[weak(rename_to = dialog)] self,
            move |selection| {
                let status = selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .map_or(BackupStatus::Locked, |backup| backup.status());

                dialog.action_set_enabled("backup.compare", dialog.can_compare() && status == BackupStatus::Modified);
                dialog.action_set_enabled("backup.open", status != BackupStatus::Locked);
            }
        ));

        // Column view activate signal
        imp.view.connect_activate(clone!(
            #[weak(rename_to = dialog)] self,
            move |_, _| {
                dialog.activate_action("backup.open", None).unwrap();
            }
        ));

        // Comparing property notify signal
        self.connect_comparing_notify(|dialog| {
            let imp = dialog.imp();

            if dialog.comparing() {
                imp.compare_button.set_icon_name("process-stop-symbolic");
                imp.compare_button.set_tooltip_text(Some("Cancel Comparison"));
            } else {
                imp.compare_button.set_icon_name("backup-compare-symbolic");
                imp.compare_button.set_tooltip_text(Some("Compare with Original"));
            }
        });
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
            #[weak(rename_to = dialog)] self,
            #[upgrade_or] false,
            move |item| {
                let search_term = dialog.imp().search_term.borrow();

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

                match dialog.search_mode() {
                    BackupSearchMode::All => {
                        is_match(&obj.path()) || is_match(&obj.package())
                    }
                    BackupSearchMode::Packages => {
                        is_match(&obj.package())
                    }
                    BackupSearchMode::Files => {
                        is_match(&obj.path())
                    }
                }
            }
        ));

        imp.status_filter.set_filter_func(clone!(
            #[weak(rename_to = dialog)] self,
            #[upgrade_or] false,
            move |item| {
                let filter = dialog.filter();

                if filter == BackupStatus::All {
                    true
                } else {
                    let obj = item
                        .downcast_ref::<BackupObject>()
                        .expect("Failed to downcast to 'BackupObject'");

                    obj.status() == filter
                }
            }
        ));

        // Set backup compare button visibility
        self.set_can_compare(Paths::paccat_bin().is_ok() && Paths::meld_bin().is_ok());

        // Set initial focus on view
        imp.view.grab_focus();
    }

    //---------------------------------------
    // Cancel compare function
    //---------------------------------------
    fn cancel_compare(&self) {
        if let Some(token) = self.imp().compare_cancel_token.take() {
            token.cancel();
        }
    }

    //---------------------------------------
    // Async compare with original function
    //---------------------------------------
    #[allow(clippy::future_not_send)]
    pub async fn compare_with_original(&self, backup: &BackupObject) -> io::Result<()> {
        let imp = self.imp();

        // Get external command paths
        let meld = Paths::meld_bin().map_err(io::Error::other)?;
        let paccat = Paths::paccat_bin().map_err(io::Error::other)?;

        // Spawn tokio task to download original file content with paccat
        let path = Pacman::config().read().unwrap().root_dir.clone() + &backup.path();
        let path_clone = path.clone();
        let package = backup.package();

        let paccat_task = TokioManager::spawn(async move |token| {
            TokioCommand::output(paccat, &[&package, "--", &path_clone], token, false).await
        });

        // Store cancel token
        imp.compare_cancel_token.replace(Some(paccat_task.cancel_token));

        // Await task
        let result = paccat_task.join_handle.await
            .expect("Failed to complete tokio task")?;

        // Remove stored cancel token
        imp.compare_cancel_token.replace(None);

        // Return if error
        let (code, content, _) = result?;

        if code != Some(0) {
            return Err(io::Error::other("paccat error"));
        }

        // Spawn tokio task to compare backup file with original content
        TokioManager::spawn(async move |_| {
            TokioCommand::spawn_pipe_stdin(meld, &["/dev/stdin", &path], &content).await
        })
        .join_handle
        .await
        .expect("Failed to complete tokio task")?
    }

    //---------------------------------------
    // Populate dialog
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
            }
        ));
    }

    //---------------------------------------
    // Show dialog
    //---------------------------------------
    pub fn show(&self, parent: Option<&impl IsA<gtk::Widget>>, pkg_model: &gio::ListStore) {
        self.present(parent);

        glib::idle_add_local_once(clone!(
            #[weak(rename_to = dialog)] self,
            #[weak] pkg_model,
            move || {
                if !dialog.is_loaded() {
                    let imp = dialog.imp();

                    imp.stack.set_visible_child_name("loading");

                    dialog.populate(&pkg_model);

                    if imp.model.n_items() == 0 {
                        imp.stack.set_visible_child_name("empty");
                    }

                    dialog.set_is_loaded(true);
                }
            }
        ));
    }
}

impl Default for BackupDialog {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
