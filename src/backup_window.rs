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
        pub header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub filter_dropdown: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub copy_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub view: TemplateChild<gtk::ListView>,
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
    pub fn new(parent: &impl IsA<gtk::Window>, pkg_model: &gio::ListStore) -> Self {
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
            Some(gtk::CallbackAction::new(clone!(@weak self as window => @default-return glib::Propagation::Proceed, move |_, _| {
                window.close();

                glib::Propagation::Proceed
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
        let mut backup_list: Vec<BackupObject> = vec![];

        pkg_model.iter::<PkgObject>()
            .flatten()
            .for_each(|pkg| {
                if !pkg.backup().is_empty() {
                    backup_list.extend(pkg.backup().iter()
                        .map(|(filename, hash)| BackupObject::new(filename, hash, Some(&pkg.name())))
                    );
                }
            });

        imp.model.extend_from_slice(&backup_list);

        // Bind backup files count to header label
        let backup_len = backup_list.len();

        imp.selection.bind_property("n-items", &imp.header_label.get(), "label")
            .transform_to(move |_, n_items: u32| Some(format!("Backup Files ({n_items} of {backup_len})")))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Bind backup files count to open/copy button states
        imp.selection.bind_property("n-items", &imp.open_button.get(), "sensitive")
            .transform_to(|_, n_items: u32| Some(n_items > 0))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        imp.selection.bind_property("n-items", &imp.copy_button.get(), "sensitive")
            .transform_to(|_, n_items: u32| Some(n_items > 0))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Set initial focus on column view
        imp.view.grab_focus();
        imp.view.scroll_to(0, gtk::ListScrollFlags::NONE, None);
    }
}
