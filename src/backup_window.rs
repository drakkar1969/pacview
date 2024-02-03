use std::collections::HashSet;

use gtk::{glib, gio};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use crate::pkg_object::PkgObject;
use crate::backup_object::BackupObject;
use crate::utils::Utils;

//------------------------------------------------------------------------------
// MODULE: BackupWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/backup_window.ui")]
    pub struct BackupWindow {
        #[template_child]
        pub filter_dropdown: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub copy_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub status_filter: TemplateChild<gtk::StringFilter>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for BackupWindow {
        const NAME: &'static str = "BackupWindow";
        type Type = super::BackupWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            BackupObject::ensure_type();

            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for BackupWindow {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
            obj.setup_shortcuts();
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
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new(parent: &gtk::Window, pkg_model: &gio::ListStore) -> Self {
        let window: Self = glib::Object::builder()
            .property("transient-for", parent)
            .build();

        window.update_ui(pkg_model);

        window
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Filter dropdown selected item property notify signal
        imp.filter_dropdown.connect_selected_item_notify(clone!(@weak imp => move |dropdown| {
            if let Some(sel) = dropdown.selected_item()
                .and_downcast::<gtk::StringObject>()
                .and_then(|obj| Some(obj.string().to_lowercase()))
            {
                if sel == "all" {
                    imp.status_filter.set_search(None);
                } else {
                    imp.status_filter.set_search(Some(&sel));
                }
            }

            imp.view.grab_focus();
        }));

        // Open button clicked signal
        imp.open_button.connect_clicked(clone!(@weak imp => move |_| {
            let item = imp.selection.selected_item()
                .and_downcast::<BackupObject>()
                .expect("Must be a 'BackupObject'");

            Utils::open_file_manager(&item.filename());
        }));

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(@weak self as window, @weak imp => move |_| {
            let copy_text = imp.selection.iter::<glib::Object>().flatten()
                .map(|item| {
                    let backup = item
                        .downcast::<BackupObject>()
                        .expect("Must be a 'BackupObject'");

                    format!("{package} => {filename} ({status})", package=backup.package().unwrap_or("None".to_string()), filename=backup.filename(), status=backup.status_text())
                })
                .collect::<Vec<String>>()
                .join("\n");

            window.clipboard().set_text(&copy_text);
        }));

        // Column view activate signal
        imp.view.connect_activate(clone!(@weak imp => move |_, _| {
            imp.open_button.emit_clicked();
        }));
    }

    //-----------------------------------
    // Setup shortcuts
    //-----------------------------------
    fn setup_shortcuts(&self) {
        // Create shortcut controller
        let controller = gtk::ShortcutController::new();

        // Add close window shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::CallbackAction::new(clone!(@weak self as window => @default-return true, move |_, _| {
                window.close();

                true
            })))
        ));

        // Add shortcut controller to window
        self.add_controller(controller);
    }

    //-----------------------------------
    // Update widgets
    //-----------------------------------
    fn update_ui(&self, pkg_model: &gio::ListStore) {
        let imp = self.imp();

        // Populate column view
        let mut backup_vec: Vec<BackupObject> = vec![];

        pkg_model.iter::<PkgObject>()
            .flatten()
            .for_each(|pkg| {
                if !pkg.backup().is_empty() {
                    backup_vec.extend(pkg.backup().iter()
                        .map(|backup| BackupObject::new(backup, Some(&pkg.name())))
                    );
                }
            });

        backup_vec.sort_unstable_by(|a, b| {
            a.package().cmp(&b.package()).then(a.filename().cmp(&b.filename()))
        });

        imp.model.extend_from_slice(&backup_vec);

        // Bind item count to window title
        imp.selection.bind_property("n-items", self, "title")
            .transform_to(move |binding, n_items: u32| {
                let selection = binding.source()
                    .and_downcast::<gtk::SingleSelection>()
                    .expect("Must be a 'SingleSelection'");

                let n_packages = selection.iter::<glib::Object>()
                    .flatten()
                    .filter_map(|item| {
                        item.downcast::<BackupObject>().ok().and_then(|obj| obj.package())
                    })
                    .collect::<HashSet<String>>()
                    .len();

                Some(format!("{n_items} Backup File{term} in {n_packages} Package{term}",
                    term=if n_packages != 1 { "s" } else { "" }))
            })
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Set initial focus on column view
        imp.view.grab_focus();
    }
}
