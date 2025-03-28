use std::cell::RefCell;

use std::collections::HashSet;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use crate::pkg_object::PkgObject;
use crate::backup_object::{BackupObject, BackupStatus};
use crate::enum_traits::EnumExt;
use crate::utils::open_with_default_app;

//------------------------------------------------------------------------------
// MODULE: BackupWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/backup_window.ui")]
    pub struct BackupWindow {
        #[template_child]
        pub(super) header_sub_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) status_dropdown: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub(super) open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) copy_button: TemplateChild<gtk::Button>,

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
        pub(super) backup_filter: TemplateChild<gtk::EveryFilter>,
        #[template_child]
        pub(super) search_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) status_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) section_sorter: TemplateChild<gtk::StringSorter>,

        pub(super) bindings: RefCell<Vec<glib::Binding>>,
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
            klass.bind_template();

            // Add find key binding
            klass.add_binding(gdk::Key::F, gdk::ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if !imp.search_entry.has_focus() {
                    imp.search_entry.grab_focus();
                }

                glib::Propagation::Stop
            });

            // Add open key binding
            klass.add_binding(gdk::Key::O, gdk::ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.open_button.is_sensitive() {
                    imp.open_button.emit_clicked();
                }

                glib::Propagation::Stop
            });

            // Add copy key binding
            klass.add_binding(gdk::Key::C, gdk::ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.copy_button.is_sensitive() {
                    imp.copy_button.emit_clicked();
                }

                glib::Propagation::Stop
            });

            // Add status key bindings
            klass.add_binding(gdk::Key::A, gdk::ModifierType::ALT_MASK, |window| {
                window.imp().status_dropdown.set_selected(BackupStatus::All.value());

                glib::Propagation::Stop
            });

            klass.add_binding(gdk::Key::M, gdk::ModifierType::ALT_MASK, |window| {
                window.imp().status_dropdown.set_selected(BackupStatus::Modified.value());

                glib::Propagation::Stop
            });

            klass.add_binding(gdk::Key::U, gdk::ModifierType::ALT_MASK, |window| {
                window.imp().status_dropdown.set_selected(BackupStatus::Unmodified.value());

                glib::Propagation::Stop
            });

            klass.add_binding(gdk::Key::L, gdk::ModifierType::ALT_MASK, |window| {
                window.imp().status_dropdown.set_selected(BackupStatus::Locked.value());

                glib::Propagation::Stop
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for BackupWindow {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
            obj.setup_controllers();
            obj.setup_signals();
        }
    }

    impl WidgetImpl for BackupWindow {}
    impl WindowImpl for BackupWindow {}
    impl AdwWindowImpl for BackupWindow {}
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
    // New function
    //---------------------------------------
    pub fn new(parent: &impl IsA<gtk::Window>) -> Self {
        glib::Object::builder()
            .property("transient-for", parent)
            .build()
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set search entry key capture widget
        imp.search_entry.set_key_capture_widget(Some(&imp.view.get()));

        // Set initial focus on view
        imp.view.grab_focus();
    }

    //---------------------------------------
    // Setup controllers
    //---------------------------------------
    fn setup_controllers(&self) {
        // Create shortcut controller
        let controller = gtk::ShortcutController::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);

        // Add close window shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::NamedAction::new("window.close"))
        ));

        // Add shortcut controller to window
        self.add_controller(controller);
    }

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
            move |entry| {
                imp.search_filter.set_search(Some(&entry.text()));
            }
        ));

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

        // Open button clicked signal
        imp.open_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                let item = imp.selection.selected_item()
                    .and_downcast::<BackupObject>()
                    .expect("Could not downcast to 'BackupObject'");

                open_with_default_app(&item.filename());
            }
        ));

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = window)] self,
            #[weak] imp,
            move |_| {
                let mut package = String::new();
                let mut body = String::new();

                for item in imp.selection.iter::<glib::Object>().flatten() {
                    let backup = item
                        .downcast::<BackupObject>()
                        .expect("Could not downcast to 'BackupObject'");

                    let backup_package = backup.package();

                    if backup_package != package {
                        body.push_str(&format!("|**{backup_package}**||\n"));

                        package = backup_package;
                    }

                    body.push_str(&format!("|{filename}|{status}|\n",
                        filename=backup.filename(),
                        status=backup.status_text()
                    ));
                }

                window.clipboard().set_text(
                    &format!("## Backup Files\n|Filename|Status|\n|---|---|\n{body}")
                );
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
    // Clear window
    //---------------------------------------
    pub fn clear(&self) {
        for binding in self.imp().bindings.take() {
            binding.unbind();
        }

        self.imp().model.remove_all();
    }

    //---------------------------------------
    // Show window
    //---------------------------------------
    pub fn show(&self, installed_pkgs: &[PkgObject]) {
        let imp = self.imp();

        self.present();

        // Populate if necessary
        if imp.model.n_items() == 0 {
            // Get backup list
            let backup_list: Vec<BackupObject> = installed_pkgs.iter()
                .flat_map(|pkg|
                    pkg.backup().iter().map(BackupObject::new)
                )
                .collect();

            // Populate column view
            imp.model.splice(0, 0, &backup_list);

            // Set status dropdown selected item
            imp.status_dropdown.set_selected(0);

            // Bind backup files count to header sub label
            let label_binding = imp.selection.bind_property("n-items", &imp.header_sub_label.get(), "label")
                .transform_to(move |binding, n_items: u32| {
                    let selection = binding.source()
                        .and_downcast::<gtk::SingleSelection>()
                        .expect("Could not downcast to 'SingleSelection'");

                    let section_map: HashSet<String> = selection.iter::<glib::Object>().flatten()
                        .map(|item| {
                            item
                                .downcast::<BackupObject>()
                                .expect("Could not downcast to 'BackupObject'")
                                .package()
                        })
                        .collect();

                    let section_len = section_map.len();

                    Some(format!("{n_items} files in {section_len} package{}",
                        if section_len != 1 {"s"} else {""}
                    ))
                })
                .sync_create()
                .build();

            // Bind selected item to open button state
            let open_binding = imp.selection.bind_property("selected-item", &imp.open_button.get(), "sensitive")
                .transform_to(|_, item: Option<glib::Object>| {
                    item.and_downcast::<BackupObject>()
                        .map_or(Some(false), |object| {
                            let status = object.status();

                            Some(status != BackupStatus::Locked && status != BackupStatus::All)
                        })
                })
                .sync_create()
                .build();

            // Bind backup files count to copy button state
            let copy_binding = imp.selection.bind_property("n-items", &imp.copy_button.get(), "sensitive")
                .transform_to(|_, n_items: u32| Some(n_items > 0))
                .sync_create()
                .build();

            imp.bindings.replace(vec![label_binding, open_binding, copy_binding]);
        }
    }
}
