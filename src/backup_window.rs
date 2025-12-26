use std::cell::Cell;
use std::fmt::Write as _;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use gdk::{Key, ModifierType};

use crate::window::{PACCAT_PATH, MELD_PATH, INSTALLED_PKGS};
use crate::backup_object::{BackupObject, BackupStatus};
use crate::enum_traits::EnumExt;
use crate::utils::app_info;

//------------------------------------------------------------------------------
// ENUM: BackupSearchMode
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "BackupSearchMode")]
pub enum BackupSearchMode {
    #[default]
    All,
    Packages,
    Files,
}

impl EnumExt for BackupSearchMode {}

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
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) status_dropdown: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub(super) compare_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) compare_image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) copy_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) scroll_window: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub(super) view: TemplateChild<gtk::ColumnView>,
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
        pub(super) status_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) section_sorter: TemplateChild<gtk::StringSorter>,

        #[template_child]
        pub(super) footer_label: TemplateChild<gtk::Label>,

        #[property(get, set, builder(BackupSearchMode::default()))]
        search_mode: Cell<BackupSearchMode>,
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
            klass.install_property_action("search.set-mode", "search-mode");
        }

        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Close window binding
            klass.add_binding_action(Key::Escape, ModifierType::NO_MODIFIER_MASK, "window.close");

            // Find key binding
            klass.add_binding(Key::F, ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if !imp.search_entry.has_focus() {
                    imp.search_entry.grab_focus();
                }

                glib::Propagation::Stop
            });

            // Compare key binding
            klass.add_binding(Key::P, ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.compare_button.is_visible() && imp.compare_button.is_sensitive() {
                    imp.compare_button.emit_clicked();
                }

                glib::Propagation::Stop
            });

            // Open key binding
            klass.add_binding(Key::O, ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.open_button.is_sensitive() {
                    imp.open_button.emit_clicked();
                }

                glib::Propagation::Stop
            });

            // Copy key binding
            klass.add_binding(Key::C, ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.copy_button.is_sensitive() {
                    imp.copy_button.emit_clicked();
                }

                glib::Propagation::Stop
            });

            // Status key bindings
            klass.add_binding(Key::A, ModifierType::ALT_MASK, |window| {
                window.imp().status_dropdown.set_selected(BackupStatus::All.value());

                glib::Propagation::Stop
            });

            klass.add_binding(Key::M, ModifierType::ALT_MASK, |window| {
                window.imp().status_dropdown.set_selected(BackupStatus::Modified.value());

                glib::Propagation::Stop
            });

            klass.add_binding(Key::U, ModifierType::ALT_MASK, |window| {
                window.imp().status_dropdown.set_selected(BackupStatus::Unmodified.value());

                glib::Propagation::Stop
            });

            klass.add_binding(Key::L, ModifierType::ALT_MASK, |window| {
                window.imp().status_dropdown.set_selected(BackupStatus::Locked.value());

                glib::Propagation::Stop
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

        // Search entry search started signal
        imp.search_entry.connect_search_started(|entry| {
            if !entry.has_focus() {
                entry.grab_focus();
            }
        });

        // Search entry search changed signal
        imp.search_entry.connect_search_changed(clone!(
            #[weak] imp,
            move |_| {
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
                imp.search_entry.set_placeholder_text(Some(&format!("Search for {}", search_mode.nick())));
            }

            imp.search_filter.changed(gtk::FilterChange::Different);
        });

        // Status dropdown selected property notify signal
        imp.status_dropdown.connect_selected_item_notify(clone!(
            #[weak] imp,
            move |dropdown| {
                let status = BackupStatus::from_repr(dropdown.selected()).unwrap_or_default();

                if status == BackupStatus::All {
                    imp.status_filter.set_search(None);
                } else {
                    imp.status_filter.set_search(Some(&status.name()));
                }

                imp.view.grab_focus();
            }
        ));

        // Compare button clicked signal
        imp.compare_button.connect_clicked(clone!(
            #[weak] imp,
            move |button| {
                let spinner = adw::SpinnerPaintable::new(Some(button));

                imp.compare_image.set_paintable(Some(&spinner));

                let item = imp.selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .expect("Failed to downcast to 'BackupObject'");

                glib::spawn_future_local(
                    async move {
                        let _ = item.compare_with_original().await;

                        imp.compare_image.set_icon_name(Some("info-compare-symbolic"));
                    }
                );
            }
        ));

        // Open button clicked signal
        imp.open_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                let backup_file = imp.selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .expect("Failed to downcast to 'BackupObject'")
                    .filename();

                glib::spawn_future_local(async move {
                    app_info::open_with_default_app(&backup_file).await;
                });
            }
        ));

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = window)] self,
            move |_| {
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

                        writeln!(output, "|{filename}|{status}|",
                            filename=backup.filename(),
                            status=backup.status_text()
                        ).unwrap();

                        writeln!(output, "|{filename}|{status}|",
                            filename=backup.filename(),
                            status=backup.status_text()
                        ).unwrap();
                    }

                window.clipboard().set_text(&output);
            }
        ));

        // Section sort model items changed signal
        imp.section_sort_model.connect_items_changed(clone!(
            #[weak] imp,
            move |sort_model, _, _, _| {
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

                imp.stack.set_visible_child_name(if n_items == 0 { "empty" } else { "view" });

                imp.footer_label.set_label(&format!("{n_items} files in {n_sections} package{}", if n_sections == 1 { "" } else { "s" }));

                imp.copy_button.set_sensitive(n_items > 0);

                let status = imp.selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .map_or(BackupStatus::All, |object| object.status());

                imp.compare_button.set_sensitive(imp.compare_button.is_visible() && status == BackupStatus::Modified);
                imp.open_button.set_sensitive(status != BackupStatus::Locked && status != BackupStatus::All);
            }
        ));

        // Selection selected item property notify signal
        imp.selection.connect_selected_item_notify(clone!(
            #[weak] imp,
            move |selection| {
                let status = selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .map_or(BackupStatus::All, |object| object.status());

                imp.compare_button.set_sensitive(imp.compare_button.is_visible() && status == BackupStatus::Modified);

                imp.open_button.set_sensitive(status != BackupStatus::Locked && status != BackupStatus::All);
            }
        ));

        // Column view activate signal
        imp.view.connect_activate(clone!(
            #[weak] imp,
            move |_, _| {
                if imp.open_button.is_sensitive() {
                    imp.open_button.emit_clicked();
                }
            }
        ));
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set search entry key capture widget
        imp.search_entry.set_key_capture_widget(Some(&imp.view.get()));

        // Set search filter function
        imp.search_filter.set_filter_func(clone!(
            #[weak(rename_to = window)] self,
            #[upgrade_or] false,
            move |item| {
                let search_term = window.imp().search_entry.text().to_lowercase();

                if search_term.is_empty() {
                    true
                } else {
                    let obj = item
                        .downcast_ref::<BackupObject>()
                        .expect("Failed to downcast to 'BackupObject'");

                    match window.search_mode() {
                        BackupSearchMode::All => {
                            obj.filename().to_lowercase().contains(&search_term)
                                || obj.package().to_lowercase().contains(&search_term)
                        },
                        BackupSearchMode::Packages => {
                            obj.package().to_lowercase().contains(&search_term)
                        },
                        BackupSearchMode::Files => {
                            obj.filename().to_lowercase().contains(&search_term)
                        },
                    }
                }
            }
        ));

        // Add keyboard shortcut to cancel search
        let shortcut = gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::CallbackAction::new(clone!(
                #[weak] imp,
                #[upgrade_or] glib::Propagation::Proceed,
                move |_, _| {
                    imp.search_entry.set_text("");
                    imp.view.grab_focus();

                    glib::Propagation::Stop
                }
            )))
        );

        let controller = gtk::ShortcutController::new();
        controller.add_shortcut(shortcut);

        imp.search_entry.add_controller(controller);

        // Set backup compare button visibility
        imp.compare_button.set_visible(PACCAT_PATH.is_ok() && MELD_PATH.is_ok());

        // Set initial focus on view
        imp.view.grab_focus();
    }

    //---------------------------------------
    // Clear window
    //---------------------------------------
    pub fn remove_all(&self) {
        self.imp().model.remove_all();
    }

    //---------------------------------------
    // Show window
    //---------------------------------------
    pub fn show(&self, parent: &impl IsA<gtk::Window>) {
        let imp = self.imp();

        self.set_transient_for(Some(parent));
        self.present();

        // Populate if necessary
        if imp.model.n_items() == 0 {
            INSTALLED_PKGS.with_borrow(|installed_pkgs| {
                // Get backup list
                let backup_list: Vec<BackupObject> = installed_pkgs.iter()
                    .flat_map(|pkg| {
                        pkg.backup().iter().map(BackupObject::new)
                    })
                    .collect();

                // Populate column view
                imp.model.splice(0, 0, &backup_list);
            });

            // Scroll to start
            glib::idle_add_local_once(clone!(
                #[weak] imp,
                move || {
                    let v_adjust = imp.scroll_window.vadjustment();
                    v_adjust.set_value(v_adjust.lower());
                }
            ));

            // Set status dropdown selected item
            imp.status_dropdown.set_selected(0);
        }
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
